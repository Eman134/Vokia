use rusqlite::Connection;

const SCHEMA: &str = include_str!("schema.sql");

// Versão gravada em schema_version para um banco novo criado a partir de schema.sql.
const BASELINE_VERSION: i64 = 19;

// Roda a cada abertura: só mexe em linhas que ainda guardam o id antigo, então repetir não muda nada.
const RENAME_LEGACY_AGENT_IDS: &str = "
    UPDATE messages SET from_agent = 'ember' WHERE from_agent = 'iara';
    UPDATE messages SET to_agent = 'ember' WHERE to_agent = 'iara';
    UPDATE agent_pauses SET agent = 'ember' WHERE agent = 'iara';
    UPDATE agent_sessions SET agent = 'ember' WHERE agent = 'iara';
    UPDATE item_plans SET agent = 'ember' WHERE agent = 'iara';
    UPDATE backlog_items SET who = 'ember' WHERE who = 'iara';
    UPDATE backlog_items SET \"by\" = 'ember' WHERE \"by\" = 'iara';
    UPDATE backlog_items SET tasks = (
        SELECT json_group_array(json(CASE WHEN json_extract(value, '$.who') = 'iara' THEN json_set(value, '$.who', 'ember') ELSE value END)) FROM json_each(backlog_items.tasks)
    ) WHERE EXISTS (SELECT 1 FROM json_each(backlog_items.tasks) WHERE json_extract(value, '$.who') = 'iara');
    UPDATE channels SET members = (
        SELECT json_group_array(CASE WHEN value = 'iara' THEN 'ember' ELSE value END) FROM json_each(channels.members)
    ) WHERE EXISTS (SELECT 1 FROM json_each(channels.members) WHERE value = 'iara');
    UPDATE orch_settings SET leader_agent = 'ember' WHERE leader_agent = 'iara';
    UPDATE orch_placements SET agent_id = 'ember' WHERE agent_id = 'iara';
    UPDATE orch_handoffs SET from_agent = 'ember' WHERE from_agent = 'iara';
    UPDATE orch_handoffs SET to_agent = 'ember' WHERE to_agent = 'iara';
";

const HAS_ARCHIVED_AT: &str =
    "SELECT COUNT(*) FROM pragma_table_info('backlog_items') WHERE name = 'archived_at'";
const ADD_ARCHIVED_AT: &str = "ALTER TABLE backlog_items ADD COLUMN archived_at TEXT";

// schema.sql só roda em banco novo; este passo leva a coluna também aos bancos já existentes.
fn ensure_archived_at(conn: &Connection) -> rusqlite::Result<()> {
    let present: i64 = conn.query_row(HAS_ARCHIVED_AT, [], |row| row.get(0))?;
    if present == 0 {
        conn.execute_batch(ADD_ARCHIVED_AT)?;
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
    tx.execute_batch(RENAME_LEGACY_AGENT_IDS)?;
    ensure_archived_at(&tx)?;
    tx.commit()
}
