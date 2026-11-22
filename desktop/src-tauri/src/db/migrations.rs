use rusqlite::{Connection, OptionalExtension, Transaction};

use crate::agents::AGENT_ID_RENAMES;
use crate::plan_proposal::parse_proposal;

use super::artifacts::replace_artifact;
use super::backlog::refresh_children;
use super::orchestration::{FLOW_END, FLOW_START, INSERT_EDGE_SQL};

const SCHEMA: &str = include_str!("schema.sql");

// Versão gravada em schema_version para um banco novo criado a partir de schema.sql.
const BASELINE_VERSION: i64 = 19;

// Ember orquestra e não ocupa time nem camada: o que sobrou dele nos dados sai, e o líder volta ao padrão.
const ORCHESTRATOR_OUT_OF_TEAMS_SQL: &str = "
    DELETE FROM orch_flow_edges WHERE from_node = 'ember' OR to_node = 'ember';
    DELETE FROM orch_placements WHERE agent_id = 'ember';
    UPDATE orch_teams SET leader_agent = NULL WHERE leader_agent = 'ember';
    UPDATE orch_settings SET leader_agent = 'axiom' WHERE leader_agent = 'ember';
";

// Roda a cada abertura: só mexe em linhas que ainda guardam o id antigo, então repetir não muda nada.
fn legacy_agent_sql() -> String {
    let renames: String = AGENT_ID_RENAMES
        .iter()
        .map(|(old, new)| rename_agent_sql(old, new))
        .collect();
    renames + ORCHESTRATOR_OUT_OF_TEAMS_SQL
}

fn rename_agent_sql(old: &str, new: &str) -> String {
    format!(
        "
    UPDATE messages SET from_agent = '{new}' WHERE from_agent = '{old}';
    UPDATE messages SET to_agent = '{new}' WHERE to_agent = '{old}';
    UPDATE agent_pauses SET agent = '{new}' WHERE agent = '{old}';
    UPDATE agent_sessions SET agent = '{new}' WHERE agent = '{old}';
    UPDATE item_plans SET agent = '{new}' WHERE agent = '{old}';
    UPDATE orch_runs SET agent = '{new}' WHERE agent = '{old}';
    UPDATE orch_settings SET leader_agent = '{new}' WHERE leader_agent = '{old}';
    UPDATE orch_teams SET leader_agent = '{new}' WHERE leader_agent = '{old}';
    UPDATE orch_placements SET agent_id = '{new}' WHERE agent_id = '{old}';
    UPDATE orch_flow_edges SET from_node = '{new}' WHERE from_node = '{old}';
    UPDATE orch_flow_edges SET to_node = '{new}' WHERE to_node = '{old}';
    UPDATE orch_handoffs SET from_agent = '{new}' WHERE from_agent = '{old}';
    UPDATE orch_handoffs SET to_agent = '{new}' WHERE to_agent = '{old}';
    UPDATE backlog_items SET who = '{new}' WHERE who = '{old}';
    UPDATE backlog_items SET \"by\" = '{new}' WHERE \"by\" = '{old}';
    UPDATE backlog_items SET tasks = (
        SELECT json_group_array(json(CASE WHEN json_extract(value, '$.who') = '{old}' THEN json_set(value, '$.who', '{new}') ELSE value END)) FROM json_each(backlog_items.tasks)
    ) WHERE EXISTS (SELECT 1 FROM json_each(backlog_items.tasks) WHERE json_extract(value, '$.who') = '{old}');
    UPDATE channels SET members = (
        SELECT json_group_array(CASE WHEN value = '{old}' THEN '{new}' ELSE value END) FROM json_each(channels.members)
    ) WHERE EXISTS (SELECT 1 FROM json_each(channels.members) WHERE value = '{old}');
"
    )
}

// Pedido em andamento de um app que fechou não tem como terminar: ao abrir, vira interrompido para o usuário decidir.
const RUNS_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS orch_runs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        agent TEXT NOT NULL,
        request TEXT NOT NULL,
        run_id INTEGER,
        item_id TEXT,
        state TEXT NOT NULL CHECK (state IN ('em_andamento','interrompido','retomado','concluido')),
        started_at INTEGER NOT NULL,
        ended_at INTEGER
    );
    CREATE INDEX IF NOT EXISTS orch_runs_by_project ON orch_runs(project_id, state);
    UPDATE orch_runs SET state = 'interrompido' WHERE state = 'em_andamento';
";

// schema.sql só roda em banco novo: estas tabelas entram a cada abertura, sem tocar nas existentes.
const PERMISSION_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS permission_denials (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        tool TEXT NOT NULL,
        command TEXT NOT NULL,
        denied_count INTEGER NOT NULL DEFAULT 0,
        last_denied_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, tool, command)
    );
    CREATE TABLE IF NOT EXISTS permission_rules (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        tool TEXT NOT NULL,
        command TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, tool, command)
    );
";

// Plano como artefato: um por item (índice único) e critérios que os agentes marcam por MCP.
const ARTIFACTS_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS artifacts (
        project_id INTEGER NOT NULL,
        id TEXT NOT NULL,
        item_id TEXT NOT NULL,
        title TEXT NOT NULL,
        summary TEXT NOT NULL DEFAULT '',
        points TEXT NOT NULL DEFAULT '[]',
        estimate TEXT NOT NULL DEFAULT '',
        risks TEXT NOT NULL DEFAULT '[]',
        tags TEXT NOT NULL DEFAULT '[]',
        detail TEXT NOT NULL,
        author TEXT,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, id),
        FOREIGN KEY (project_id, item_id) REFERENCES backlog_items(project_id, id) ON DELETE CASCADE
    );
    CREATE UNIQUE INDEX IF NOT EXISTS artifacts_by_item ON artifacts(project_id, item_id);
    CREATE TABLE IF NOT EXISTS artifact_criteria (
        project_id INTEGER NOT NULL,
        artifact_id TEXT NOT NULL,
        id TEXT NOT NULL,
        position INTEGER NOT NULL,
        text TEXT NOT NULL,
        done INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0,1)),
        PRIMARY KEY (project_id, artifact_id, id),
        FOREIGN KEY (project_id, artifact_id) REFERENCES artifacts(project_id, id) ON DELETE CASCADE
    );
";

// Documentos livres do projeto; o tipo e o limite de cada campo valem também no banco.
const ARTIFACT_DOCUMENTS_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS artifact_documents (
        project_id INTEGER NOT NULL,
        id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('documento','especificacao','checklist','arquitetura','decisao')),
        seq INTEGER NOT NULL,
        title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 200),
        summary TEXT NOT NULL DEFAULT '' CHECK (length(summary) <= 500),
        body TEXT NOT NULL DEFAULT '' CHECK (length(body) <= 100000),
        tags TEXT NOT NULL DEFAULT '[]',
        author TEXT,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, id),
        UNIQUE (project_id, kind, seq),
        FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
    );
";

const ATTACHMENTS_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS attachments (
        project_id INTEGER NOT NULL,
        id TEXT NOT NULL,
        name TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('image', 'pdf', 'file')),
        size INTEGER NOT NULL,
        path TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        sent_at INTEGER,
        PRIMARY KEY (project_id, id),
        FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
    );
";

// Planos antigos viram artefato, mas não ganham etapas: o item pode já ter sido executado (P3).
const PLANS_WITHOUT_ARTIFACT_SQL: &str = "SELECT saved.project_id, saved.item_id, saved.text, saved.agent, card.title \
    FROM item_plans AS saved \
    JOIN backlog_items AS card ON card.project_id = saved.project_id AND card.id = saved.item_id \
    WHERE trim(COALESCE(saved.text, '')) <> '' AND NOT EXISTS (SELECT 1 FROM artifacts AS artifact \
        WHERE artifact.project_id = saved.project_id AND artifact.item_id = saved.item_id)";

// CASE em cascata: json_type falha em JSON inválido, e só a primeira condição verdadeira é avaliada.
const VALID_TASKS: &str = "CASE WHEN NOT json_valid(item.tasks) THEN '[]' \
    WHEN json_type(item.tasks) = 'array' THEN item.tasks ELSE '[]' END";

const MICRO_TASK_PARENTS_SQL: &str = "SELECT item.project_id, item.id FROM backlog_items AS item \
    WHERE json_array_length({tasks}) > 0";

const MICRO_TASK_CARDS_SQL: &str = "INSERT INTO backlog_items (project_id, id, title, \"type\", state, \
        parent, description, who, \"by\", is_new, \"column\", origin, position) \
    SELECT item.project_id, \
        'micro-' || item.id || '.' || (task.key + 1), \
        json_extract(task.value, '$.title'), \
        item.\"type\", 'vazio', item.id, \
        NULLIF(trim(COALESCE(json_extract(task.value, '$.script.context'), '')), ''), \
        COALESCE(json_extract(task.value, '$.who'), item.who), \
        COALESCE(json_extract(task.value, '$.who'), item.who), \
        0, \
        CASE json_extract(task.value, '$.status') \
            WHEN 'ready' THEN 'Ready' WHEN 'em andamento' THEN 'In progress' \
            WHEN 'em revisão' THEN 'In review' WHEN 'feito' THEN 'Done' ELSE 'Backlog' END, \
        'agente', \
        (SELECT COALESCE(MAX(last.position), 0) FROM backlog_items AS last WHERE last.project_id = item.project_id) \
            + ROW_NUMBER() OVER (PARTITION BY item.project_id ORDER BY item.position, task.key) \
    FROM backlog_items AS item, json_each({tasks}) AS task \
    WHERE trim(COALESCE(json_extract(task.value, '$.title'), '')) <> '' \
    ON CONFLICT(project_id, id) DO NOTHING";

const CLEAR_MICRO_TASKS_SQL: &str = "UPDATE backlog_items AS item SET tasks = '[]' \
    WHERE json_array_length({tasks}) > 0";

// Fluxo por agente dentro do time: cada seta liga dois agentes do mesmo time, ou o início a um agente, ou um agente ao fim.
// A seta de início é única por time e o líder do time é o destino dela.
const FLOW_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS orch_flow_edges (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        team_id TEXT NOT NULL REFERENCES orch_teams(id) ON DELETE CASCADE,
        from_node TEXT NOT NULL,
        to_node TEXT NOT NULL,
        link TEXT NOT NULL DEFAULT 'fluxo' CHECK (link IN ('fluxo','comunicacao')),
        sync_mode TEXT NOT NULL DEFAULT 'sincrona' CHECK (sync_mode IN ('sincrona','assincrona')),
        when_event TEXT NOT NULL CHECK (when_event IN ('plano_aprovado','tarefa_concluida','revisao_reprovou','testes_falharam','revisao_aprovou','pedido_de_ajuda')),
        is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0,1)),
        action TEXT NOT NULL CHECK (action IN ('delegar','devolver','escalar','consultar')),
        kind TEXT NOT NULL CHECK (kind IN ('planejar','implementar','testar','revisar','pesquisar')),
        turn_limit INTEGER CHECK (turn_limit IS NULL OR turn_limit BETWEEN 1 AND 50),
        curve REAL NOT NULL DEFAULT 0,
        position INTEGER NOT NULL,
        CHECK (from_node <> to_node),
        CHECK (to_node <> '{start}'),
        CHECK (from_node <> '{end}'),
        CHECK ((action = 'escalar') = (to_node = '{end}')),
        UNIQUE (team_id, from_node, to_node)
    );
    CREATE UNIQUE INDEX IF NOT EXISTS orch_flow_start ON orch_flow_edges(team_id) WHERE from_node = '{start}';
    CREATE TABLE IF NOT EXISTS orch_flow_gateways (
        id TEXT PRIMARY KEY CHECK (id LIKE 'gw-%'),
        team_id TEXT NOT NULL REFERENCES orch_teams(id) ON DELETE CASCADE,
        kind TEXT NOT NULL CHECK (kind IN ('paralelo','exclusivo')),
        pos_x REAL NOT NULL,
        pos_y REAL NOT NULL
    );
";

// Cada agente posicionado na camada de origem da regra manda para cada agente da camada de destino no mesmo time;
// destino "voce" vira o fim; devolver leva o limite da regra. A regra em paralelo com 2 ou mais destinos no time
// não entra aqui: ela vira um gateway paralelo em migrate_parallel_rules.
const FLOW_FROM_RULES_SQL: &str = "
    INSERT INTO orch_flow_edges (team_id, from_node, to_node, link, sync_mode, when_event, is_default,
        action, kind, turn_limit, curve, position)
        SELECT a.team_id, a.agent_id, CASE WHEN r.to_layer = 'voce' THEN '{end}' ELSE b.agent_id END,
            'fluxo', 'sincrona',
            r.when_event, 0, r.action, r.kind,
            CASE WHEN r.action = 'devolver' THEN r.return_limit END, 0,
            ROW_NUMBER() OVER (PARTITION BY a.team_id ORDER BY r.position, a.agent_id, b.agent_id) - 1
        FROM orch_rules r
        JOIN orch_placements a ON a.layer_id = r.from_layer
        LEFT JOIN orch_placements b ON b.layer_id = r.to_layer AND b.team_id = a.team_id
        WHERE (r.to_layer = 'voce' OR b.agent_id IS NOT NULL)
          AND NOT (r.exec_mode = 'paralelo' AND r.to_layer <> 'voce'
               AND (SELECT COUNT(*) FROM orch_placements d
                    WHERE d.layer_id = r.to_layer AND d.team_id = a.team_id) >= 2);
";

// Líder e início andam juntos: só o líder que está posicionado no próprio time ganha a seta de início.
const FLOW_START_FROM_LEADERS_SQL: &str = "
    INSERT INTO orch_flow_edges (team_id, from_node, to_node, when_event, action, kind, position)
        SELECT t.id, '{start}', t.leader_agent, 'plano_aprovado', 'delegar', 'planejar',
            (SELECT COALESCE(MAX(e.position) + 1, 0) FROM orch_flow_edges e WHERE e.team_id = t.id)
        FROM orch_teams t
        WHERE t.leader_agent IS NOT NULL
          AND EXISTS (SELECT 1 FROM orch_placements p WHERE p.team_id = t.id AND p.agent_id = t.leader_agent);
";

// Grade da tela: y por ordem da camada (120 + 150 por camada) e x em passos de 268 a partir de 300 entre os agentes do time na camada.
const FLOW_POSITIONS_SQL: &str = "
    UPDATE orch_placements SET
        pos_y = 120.0 + 150.0 * (
            SELECT COUNT(*) FROM orch_layers own, orch_layers other
            WHERE own.id = orch_placements.layer_id
              AND (other.position < own.position OR (other.position = own.position AND other.id < own.id))),
        pos_x = MIN(300.0 + 268.0 * (SELECT COUNT(*) FROM orch_placements p WHERE p.team_id = orch_placements.team_id
                AND p.layer_id = orch_placements.layer_id AND p.agent_id < orch_placements.agent_id), 1484.0);
";

const HAS_COLUMN: &str = "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2";

// orch_rules só existe aqui para migrar: o banco por projeto ainda a tem, e a migração do fluxo a lê antes de apagá-la.
const ORCH_RULES_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS orch_rules (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        from_layer TEXT NOT NULL REFERENCES orch_layers(id) ON DELETE CASCADE,
        to_layer TEXT NOT NULL,
        when_event TEXT NOT NULL,
        action TEXT NOT NULL,
        kind TEXT NOT NULL,
        exec_mode TEXT NOT NULL,
        return_limit INTEGER NOT NULL,
        position INTEGER NOT NULL,
        UNIQUE (from_layer, to_layer)
    );
";

const ORCH_TABLES: [&str; 5] = [
    "orch_settings",
    "orch_layers",
    "orch_teams",
    "orch_placements",
    "orch_rules",
];

// Projeto de origem da orquestração global: o selecionado, senão o fixado, senão o menor id; os demais são descartados.
const ORCH_SOURCE_PROJECT_SQL: &str = "
    SELECT s.project_id FROM orch_settings s JOIN projects p ON p.id = s.project_id
    ORDER BY p.selected DESC, p.pinned DESC, p.id LIMIT 1
";

// Ember não ocupa posição nem lidera: o que o dado antigo guardava dele não é copiado.
const ORCH_COPY_SQL: &str = "
    INSERT INTO orch_settings (id, enforcement, leader_agent, step_mode, human_gate, max_parallel,
        pair_turn_limit, task_turn_limit, worktree_policy)
        SELECT 1, enforcement, CASE WHEN leader_agent = 'ember' THEN 'axiom' ELSE leader_agent END,
            step_mode, human_gate, max_parallel, pair_turn_limit, task_turn_limit, worktree_policy
        FROM orch_settings_legacy WHERE project_id = {project_id};
    INSERT INTO orch_layers (id, name, description, position, kind, instructions)
        SELECT id, name, description, position, kind, instructions
        FROM orch_layers_legacy WHERE project_id = {project_id};
    INSERT INTO orch_teams (id, name, position, leader_agent)
        SELECT id, name, position, CASE WHEN leader_agent = 'ember' THEN NULL ELSE leader_agent END
        FROM orch_teams_legacy WHERE project_id = {project_id};
    INSERT INTO orch_placements (agent_id, team_id, layer_id, instructions, max_instances)
        SELECT agent_id, team_id, layer_id, instructions, max_instances
        FROM orch_placements_legacy WHERE project_id = {project_id} AND agent_id <> 'ember';
    INSERT INTO orch_rules (id, from_layer, to_layer, when_event, action, kind, exec_mode, return_limit, position)
        SELECT id, from_layer, to_layer, when_event, action, kind, exec_mode, return_limit, position
        FROM orch_rules_legacy WHERE project_id = {project_id};
";

// Banco de versão anterior guarda a orquestração por projeto. A reconstrução roda só enquanto a coluna existir,
// então reabrir o app não repete nada. schema.sql recria as tabelas globais porque os nomes ficam livres.
fn make_orchestration_global(tx: &Transaction) -> rusqlite::Result<()> {
    let per_project: i64 =
        tx.query_row(HAS_COLUMN, ["orch_layers", "project_id"], |row| row.get(0))?;
    if per_project == 0 {
        return Ok(());
    }
    let source: Option<i64> = tx
        .query_row(ORCH_SOURCE_PROJECT_SQL, [], |row| row.get(0))
        .optional()?;
    for table in ORCH_TABLES {
        tx.execute_batch(&format!("ALTER TABLE {table} RENAME TO {table}_legacy"))?;
    }
    tx.execute_batch(SCHEMA)?;
    tx.execute_batch(ORCH_RULES_SCHEMA)?;
    if let Some(project_id) = source {
        tx.execute_batch(&ORCH_COPY_SQL.replace("{project_id}", &project_id.to_string()))?;
    }
    for table in ORCH_TABLES.iter().rev() {
        tx.execute_batch(&format!("DROP TABLE {table}_legacy"))?;
    }
    Ok(())
}

// schema.sql só roda em banco novo; cada coluna nova passa por aqui em toda abertura e entra só se faltar.
fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    declaration: &str,
) -> rusqlite::Result<()> {
    let present: i64 = conn.query_row(HAS_COLUMN, [table, column], |row| row.get(0))?;
    if present == 0 {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {declaration}"
        ))?;
    }
    Ok(())
}

fn table_exists(tx: &Connection, table: &str) -> rusqlite::Result<bool> {
    tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
}

fn with_flow_nodes(sql: &str) -> String {
    sql.replace("{start}", FLOW_START)
        .replace("{end}", FLOW_END)
}

// Roda uma vez, enquanto orch_flow_gateways ainda não existe: as regras de camada e os líderes viram arestas (e gateways)
// e os agentes ganham posição. As arestas de antes dos gateways eram só derivadas das regras, então são refeitas.
// Depois disso o fluxo é só do usuário, então reabrir o app não recria nada. orch_rules continua até a política ler as arestas.
struct ParallelSender {
    team_id: String,
    agent_id: String,
    pos_y: f64,
    when_event: String,
    action: String,
    kind: String,
    to_layer: String,
    return_limit: i64,
}

fn insert_migrated_edge(
    tx: &Transaction,
    team_id: &str,
    from_node: &str,
    to_node: &str,
    rule: (&str, &str, &str, Option<i64>),
) -> rusqlite::Result<()> {
    let (when_event, action, kind, turn_limit) = rule;
    tx.execute(
        INSERT_EDGE_SQL,
        rusqlite::params![
            team_id, from_node, to_node, "fluxo", "sincrona", when_event, false, action, kind,
            turn_limit, 0.0, ""
        ],
    )?;
    Ok(())
}

// Regra em paralelo para 2 ou mais agentes do time: o remetente liga a um gateway paralelo, a meio caminho entre ele
// e os destinos, e o gateway entrega a cada destino.
fn migrate_parallel_rules(tx: &Transaction) -> rusqlite::Result<()> {
    let senders: Vec<ParallelSender> = {
        let mut statement = tx.prepare(
            "SELECT a.team_id, a.agent_id, a.pos_y, r.when_event, r.action, r.kind, r.to_layer, r.return_limit \
             FROM orch_rules r JOIN orch_placements a ON a.layer_id = r.from_layer \
             WHERE r.exec_mode = 'paralelo' AND r.to_layer <> 'voce' ORDER BY r.position, a.agent_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ParallelSender {
                team_id: row.get(0)?,
                agent_id: row.get(1)?,
                pos_y: row.get(2)?,
                when_event: row.get(3)?,
                action: row.get(4)?,
                kind: row.get(5)?,
                to_layer: row.get(6)?,
                return_limit: row.get(7)?,
            })
        })?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut next_gateway = 1;
    for sender in senders {
        let destinations: Vec<(String, f64, f64)> = {
            let mut statement = tx.prepare(
                "SELECT agent_id, pos_x, pos_y FROM orch_placements \
                 WHERE layer_id = ?1 AND team_id = ?2 ORDER BY agent_id",
            )?;
            let rows = statement.query_map([&sender.to_layer, &sender.team_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        if destinations.len() < 2 {
            continue;
        }
        let count = destinations.len() as f64;
        let center_x = destinations.iter().map(|(_, x, _)| x).sum::<f64>() / count;
        let center_y = destinations.iter().map(|(_, _, y)| y).sum::<f64>() / count;
        let gateway = format!("gw-{next_gateway}");
        next_gateway += 1;
        tx.execute(
            "INSERT INTO orch_flow_gateways (id, team_id, kind, pos_x, pos_y) VALUES (?1, ?2, 'paralelo', ?3, ?4)",
            rusqlite::params![gateway, sender.team_id, center_x, (sender.pos_y + center_y) / 2.0],
        )?;
        let turn_limit = (sender.action == "devolver").then_some(sender.return_limit);
        insert_migrated_edge(
            tx,
            &sender.team_id,
            &sender.agent_id,
            &gateway,
            (&sender.when_event, &sender.action, &sender.kind, turn_limit),
        )?;
        for (destination, _, _) in &destinations {
            insert_migrated_edge(
                tx,
                &sender.team_id,
                &gateway,
                destination,
                ("tarefa_concluida", "delegar", &sender.kind, None),
            )?;
        }
    }
    Ok(())
}

// A tabela antiga só aceitava delegar, devolver e escalar, e não guardava a seta da passagem: ela é refeita com as
// linhas antigas copiadas (edge_id nulo). Roda só enquanto a restrição antiga existir.
fn rebuild_handoffs_with_flow(tx: &Transaction) -> rusqlite::Result<()> {
    let definition: String = tx.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'orch_handoffs'",
        [],
        |row| row.get(0),
    )?;
    if definition.contains("'consultar'") {
        return Ok(());
    }
    tx.execute_batch(
        "ALTER TABLE orch_handoffs RENAME TO orch_handoffs_legacy;
         DROP INDEX IF EXISTS orch_handoffs_by_project;",
    )?;
    tx.execute_batch(SCHEMA)?;
    tx.execute_batch(
        "INSERT INTO orch_handoffs (id, project_id, run_id, from_agent, to_agent, from_layer, to_layer,
            action, kind, item_id, return_count, message_id, created_at, event)
            SELECT id, project_id, run_id, from_agent, to_agent, from_layer, to_layer,
                action, kind, item_id, return_count, message_id, created_at, event
            FROM orch_handoffs_legacy;
         DROP TABLE orch_handoffs_legacy;",
    )
}

fn migrate_flow(tx: &Transaction) -> rusqlite::Result<()> {
    let migrated = table_exists(tx, "orch_flow_gateways")?;
    ensure_column(tx, "orch_placements", "pos_x", "REAL NOT NULL DEFAULT 0")?;
    ensure_column(tx, "orch_placements", "pos_y", "REAL NOT NULL DEFAULT 0")?;
    ensure_column(
        tx,
        "orch_placements",
        "criteria",
        "TEXT NOT NULL DEFAULT '' CHECK (length(criteria) <= 4000)",
    )?;
    ensure_column(
        tx,
        "orch_placements",
        "max_worktrees",
        "INTEGER CHECK (max_worktrees IS NULL OR max_worktrees BETWEEN 1 AND 8)",
    )?;
    ensure_column(tx, "orch_teams", "start_x", "REAL")?;
    ensure_column(tx, "orch_teams", "start_y", "REAL")?;
    ensure_column(tx, "orch_teams", "end_x", "REAL")?;
    ensure_column(tx, "orch_teams", "end_y", "REAL")?;
    ensure_column(
        tx,
        "orch_teams",
        "final_validation",
        "TEXT NOT NULL DEFAULT 'manual' CHECK (final_validation IN ('manual','automatica'))",
    )?;
    ensure_column(tx, "backlog_items", "closed_by", "TEXT")?;
    ensure_column(
        tx,
        "backlog_items",
        "auto_attempts",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(tx, "backlog_items", "cycle_started_at", "INTEGER")?;
    rebuild_handoffs_with_flow(tx)?;
    tx.execute_batch(&with_flow_nodes(FLOW_SCHEMA))?;
    ensure_column(
        tx,
        "orch_flow_edges",
        "instructions",
        "TEXT NOT NULL DEFAULT '' CHECK (length(instructions) <= 4000)",
    )?;
    if migrated {
        return Ok(());
    }
    if !table_exists(tx, "orch_rules")? {
        return Ok(());
    }
    tx.execute_batch("DELETE FROM orch_flow_edges")?;
    tx.execute_batch(FLOW_POSITIONS_SQL)?;
    tx.execute_batch(&with_flow_nodes(FLOW_FROM_RULES_SQL))?;
    migrate_parallel_rules(tx)?;
    tx.execute_batch(&with_flow_nodes(FLOW_START_FROM_LEADERS_SQL))
}

fn migrate_plan_artifacts(tx: &Transaction) -> rusqlite::Result<()> {
    let pending = {
        let mut statement = tx.prepare(PLANS_WITHOUT_ARTIFACT_SQL)?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (project_id, item_id, text, agent, title) in pending {
        let draft = parse_proposal(&text);
        replace_artifact(tx, project_id, &item_id, &title, &text, agent.as_deref(), &draft)?;
    }
    Ok(())
}

// Micro-tarefa vira card filho do item (D10); id derivado e lista esvaziada tornam a migração repetível.
fn migrate_micro_tasks(tx: &Transaction) -> rusqlite::Result<()> {
    let parents = {
        let mut statement = tx.prepare(&MICRO_TASK_PARENTS_SQL.replace("{tasks}", VALID_TASKS))?;
        let rows = statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    tx.execute_batch(&MICRO_TASK_CARDS_SQL.replace("{tasks}", VALID_TASKS))?;
    tx.execute_batch(&CLEAR_MICRO_TASKS_SQL.replace("{tasks}", VALID_TASKS))?;
    for (project_id, item_id) in parents {
        refresh_children(tx, project_id, &item_id)?;
    }
    Ok(())
}

pub(super) fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")?;
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )?;
    if current == 0 {
        let tx = conn.transaction()?;
        tx.execute_batch(SCHEMA)?;
        tx.execute(
            "INSERT INTO schema_version(version) VALUES (?1)",
            [BASELINE_VERSION],
        )?;
        tx.commit()?;
    } else if current < BASELINE_VERSION {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "banco na versão {current}; a versão mínima suportada é {BASELINE_VERSION}"
        )));
    }
    let tx = conn.transaction()?;
    tx.execute_batch(RUNS_SCHEMA)?;
    tx.execute_batch(PERMISSION_SCHEMA)?;
    tx.execute_batch(ARTIFACTS_SCHEMA)?;
    tx.execute_batch(ARTIFACT_DOCUMENTS_SCHEMA)?;
    tx.execute_batch(ATTACHMENTS_SCHEMA)?;
    ensure_column(&tx, "backlog_items", "archived_at", "TEXT")?;
    ensure_column(&tx, "orch_handoffs", "event", "TEXT")?;
    ensure_column(&tx, "backlog_items", "branch", "TEXT")?;
    ensure_column(&tx, "backlog_items", "worktree_path", "TEXT")?;
    ensure_column(&tx, "backlog_items", "plan_note", "TEXT")?;
    ensure_column(&tx, "projects", "setup_command", "TEXT")?;
    ensure_column(&tx, "orch_teams", "leader_agent", "TEXT")?;
    ensure_column(&tx, "messages", "usage", "TEXT")?;
    ensure_column(&tx, "messages", "task_result", "TEXT")?;
    make_orchestration_global(&tx)?;
    migrate_flow(&tx)?;
    tx.execute_batch("DROP TABLE IF EXISTS orch_rules")?;
    tx.execute_batch(&legacy_agent_sql())?;
    migrate_plan_artifacts(&tx)?;
    migrate_micro_tasks(&tx)?;
    tx.commit()
}
