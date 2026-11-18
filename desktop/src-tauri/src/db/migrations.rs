use rusqlite::Connection;

use crate::agents::AGENT_ID_RENAMES;

const SCHEMA: &str = include_str!("schema.sql");

// Versão gravada em schema_version para um banco novo criado a partir de schema.sql.
const BASELINE_VERSION: i64 = 19;

// Ember orquestra e não ocupa time nem camada: o que sobrou dele nos dados sai, e o líder volta ao padrão.
const ORCHESTRATOR_OUT_OF_TEAMS_SQL: &str = "
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

const HAS_COLUMN: &str = "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2";

// schema.sql só roda em banco novo; cada coluna nova passa por aqui em toda abertura e entra só se faltar.
fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    declaration: &str,
) -> rusqlite::Result<()> {
    let present: i64 = conn.query_row(HAS_COLUMN, [table, column], |row| row.get(0))?;
    if present == 0 {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {declaration}"))?;
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
    ensure_column(&tx, "backlog_items", "archived_at", "TEXT")?;
    ensure_column(&tx, "orch_handoffs", "event", "TEXT")?;
    ensure_column(&tx, "backlog_items", "branch", "TEXT")?;
    ensure_column(&tx, "backlog_items", "worktree_path", "TEXT")?;
    ensure_column(&tx, "projects", "setup_command", "TEXT")?;
    ensure_column(&tx, "orch_teams", "leader_agent", "TEXT")?;
    tx.execute_batch(&legacy_agent_sql())?;
    tx.commit()
}
