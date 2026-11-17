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
    db.insert_message(1, Speaker::User, Speaker::User, "nova", 20, Some(5), None)
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
