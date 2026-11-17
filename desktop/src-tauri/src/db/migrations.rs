use rusqlite::Connection;

// Cada item é uma migração; a versão é a posição + 1. Nunca editar um item já publicado: acrescentar outro.
const MIGRATIONS: &[&str] = &[
    "
    CREATE TABLE projects (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        path TEXT NOT NULL UNIQUE,
        selected INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
    );
    CREATE UNIQUE INDEX projects_one_selected ON projects(selected) WHERE selected = 1;
    CREATE TABLE messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL REFERENCES projects(id),
        from_agent TEXT NOT NULL,
        to_agent TEXT NOT NULL,
        text TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX messages_by_project ON messages(project_id, id);
",
    "
    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
",
    "
    ALTER TABLE projects ADD COLUMN symbol TEXT NOT NULL DEFAULT '';
    ALTER TABLE projects ADD COLUMN description TEXT NOT NULL DEFAULT '';
    ALTER TABLE projects ADD COLUMN origin_kind TEXT NOT NULL DEFAULT 'folder';
    ALTER TABLE projects ADD COLUMN origin_repo TEXT;
",
    "
    CREATE TABLE agent_pauses (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        agent TEXT NOT NULL,
        paused_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
        PRIMARY KEY (project_id, agent)
    );
",
    "
    CREATE TABLE permission_approvals (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        tool TEXT NOT NULL,
        command TEXT NOT NULL,
        approved_count INTEGER NOT NULL DEFAULT 0,
        last_approved_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, tool, command)
    );
",
    "
    CREATE TABLE agent_sessions (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        agent TEXT NOT NULL,
        session_id TEXT NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, agent)
    );
",
    "
    CREATE TABLE backlog_items (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        id TEXT NOT NULL,
        title TEXT NOT NULL,
        \"type\" TEXT NOT NULL,
        state TEXT NOT NULL,
        stage TEXT,
        triage_step INTEGER,
        parent TEXT,
        feature TEXT,
        description TEXT,
        plan TEXT,
        who TEXT NOT NULL,
        \"by\" TEXT NOT NULL,
        is_new INTEGER,
        \"column\" TEXT NOT NULL DEFAULT 'Backlog',
        priority TEXT,
        estimate TEXT,
        tags TEXT NOT NULL DEFAULT '[]',
        tasks TEXT NOT NULL DEFAULT '[]',
        criteria TEXT NOT NULL DEFAULT '[]',
        questions TEXT NOT NULL DEFAULT '[]',
        links TEXT NOT NULL DEFAULT '[]',
        activity TEXT NOT NULL DEFAULT '[]',
        children_total INTEGER,
        children_done INTEGER,
        position INTEGER NOT NULL,
        PRIMARY KEY (project_id, id)
    );
    CREATE INDEX backlog_items_by_position ON backlog_items(project_id, position);
    CREATE TABLE item_plans (
        project_id INTEGER NOT NULL,
        item_id TEXT NOT NULL,
        state TEXT NOT NULL,
        text TEXT,
        plan_id TEXT,
        plan_name TEXT,
        plan_meta TEXT,
        agent TEXT,
        linked_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, item_id),
        FOREIGN KEY (project_id, item_id) REFERENCES backlog_items(project_id, id) ON DELETE CASCADE
    );
",
    "
    ALTER TABLE projects ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
    CREATE UNIQUE INDEX projects_one_pinned ON projects(pinned) WHERE pinned = 1;
",
    "
    DELETE FROM item_plans WHERE project_id IN (SELECT id FROM projects WHERE path = '');
    DELETE FROM backlog_items WHERE project_id IN (SELECT id FROM projects WHERE path = '');
    DELETE FROM messages WHERE project_id IN (SELECT id FROM projects WHERE path = '');
    DELETE FROM agent_pauses WHERE project_id IN (SELECT id FROM projects WHERE path = '');
    DELETE FROM permission_approvals WHERE project_id IN (SELECT id FROM projects WHERE path = '');
    DELETE FROM agent_sessions WHERE project_id IN (SELECT id FROM projects WHERE path = '');
    DELETE FROM projects WHERE path = '';
    UPDATE projects SET selected = 1 WHERE id = (SELECT MIN(id) FROM projects)
        AND NOT EXISTS (SELECT 1 FROM projects WHERE selected = 1);
",
    "
    ALTER TABLE projects ADD COLUMN permission_level TEXT NOT NULL DEFAULT 'liberado' CHECK (permission_level IN ('liberado','edicao','leitura'));
",
    "
    UPDATE agent_sessions SET agent = lower(agent);
",
    "
    CREATE TABLE channels (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL REFERENCES projects(id),
        name TEXT NOT NULL,
        members TEXT NOT NULL,
        created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
        UNIQUE (project_id, name)
    );
",
    "
    ALTER TABLE backlog_items ADD COLUMN origin TEXT NOT NULL DEFAULT 'usuario';
",
    "
    ALTER TABLE messages ADD COLUMN channel_id INTEGER;
",
    "
    ALTER TABLE messages ADD COLUMN activity TEXT;
",
    "
    ALTER TABLE projects ADD COLUMN color TEXT;
",
    "
    ALTER TABLE messages ADD COLUMN request_proposal TEXT;
",
];

pub(super) fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")?;
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )?;
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_version(version) VALUES (?1)",
            [index as i64 + 1],
        )?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/migrations_tests.rs"]
mod tests;
