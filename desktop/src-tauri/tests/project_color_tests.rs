use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDb {
    dir: PathBuf,
    db: Db,
}

impl TempDb {
    fn open() -> TempDb {
        let dir = std::env::temp_dir().join(format!(
            "vokia-project-color-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        let db = Db::open(&dir).expect("o banco de teste deveria abrir");
        TempDb { dir, db }
    }

    fn project(&self) -> Project {
        self.db
            .insert_project(&NewProject {
                name: "alfa",
                symbol: "folder",
                description: "",
                path: "C:/projetos/alfa",
                origin_kind: ORIGIN_FOLDER,
                origin_repo: None,
            })
            .expect("o projeto deveria ser criado")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn edit(color: &str) -> ProjectEdit {
    ProjectEdit {
        name: "alfa".into(),
        symbol: "folder".into(),
        description: String::new(),
        color: color.into(),
        setup_command: None,
        teardown_command: None,
    }
}

#[test]
fn projeto_novo_nasce_sem_cor_gravada() {
    let temp = TempDb::open();

    assert_eq!(temp.project().color, None);
}

#[test]
fn cor_da_paleta_fica_gravada_e_volta_na_listagem() {
    let temp = TempDb::open();
    let id = temp.project().id;

    update_project(&temp.db, id, edit("teal")).expect("a cor da paleta deveria ser aceita");

    assert_eq!(
        temp.db.project(id).unwrap().color.as_deref(),
        Some("teal")
    );
}

#[test]
fn cor_fora_da_paleta_e_recusada_e_a_gravada_continua() {
    let temp = TempDb::open();
    let id = temp.project().id;
    update_project(&temp.db, id, edit("blue")).unwrap();

    for invalid in ["", "#ff0000", "Blue", "magenta"] {
        let error = update_project(&temp.db, id, edit(invalid))
            .expect_err("a cor fora da paleta deveria ser recusada");
        assert_eq!(
            serde_json::to_value(&error).unwrap()["code"],
            "InvalidColor",
            "cor {invalid:?}"
        );
    }

    assert_eq!(
        temp.db.project(id).unwrap().color.as_deref(),
        Some("blue")
    );
}
