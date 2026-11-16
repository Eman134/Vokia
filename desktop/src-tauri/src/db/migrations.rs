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
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::db::connection::{Db, DB_FILE};
    use crate::events::{Speaker, VokiaEvent};

    static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "vokia-migrations-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ))
    }

    // Banco como estava antes da migração do canal: todas as migrações, menos a última.
    fn seed_database_without_channel_column(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        let conn = Connection::open(dir.join(DB_FILE)).unwrap();
        conn.execute_batch("CREATE TABLE schema_version (version INTEGER NOT NULL)")
            .unwrap();
        for (index, sql) in MIGRATIONS[..MIGRATIONS.len() - 1].iter().enumerate() {
            conn.execute_batch(sql).unwrap();
            conn.execute(
                "INSERT INTO schema_version(version) VALUES (?1)",
                [index as i64 + 1],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO projects(id, name, path) VALUES (1, 'antigo', 'C:/antigo')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages(project_id, from_agent, to_agent, text, created_at) VALUES (1, 'user', 'davi', 'mensagem antiga', 10)",
            [],
        )
        .unwrap();
    }

    fn channels_of(history: &[VokiaEvent]) -> Vec<Option<i64>> {
        history
            .iter()
            .map(|event| match event {
                VokiaEvent::Message { channel_id, .. } => *channel_id,
                other => panic!("esperava mensagem, veio {other:?}"),
            })
            .collect()
    }

    #[test]
    fn banco_sem_a_coluna_de_canal_abre_e_devolve_o_historico_antigo_sem_marca() {
        let dir = temp_dir();
        seed_database_without_channel_column(&dir);

        let db = Db::open(&dir).expect("o banco antigo deveria migrar sem erro");
        db.insert_message(1, Speaker::User, Speaker::User, "nova", 20, Some(5))
            .unwrap();
        let history = db.load_history(1).unwrap();
        drop(db);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(channels_of(&history), vec![None, Some(5)]);
    }

    #[test]
    fn migrar_de_novo_nao_repete_nem_quebra_e_a_versao_chega_ao_total() {
        let dir = temp_dir();
        seed_database_without_channel_column(&dir);
        let mut conn = Connection::open(dir.join(DB_FILE)).unwrap();

        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        let (rows, latest): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), MAX(version) FROM schema_version",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        drop(conn);
        let _ = fs::remove_dir_all(&dir);

        let total = MIGRATIONS.len() as i64;
        assert_eq!((rows, latest), (total, total));
    }
}
