use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::db::projects::NewProject;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDb {
    dir: PathBuf,
    db: Db,
}

impl TempDb {
    fn open() -> TempDb {
        let dir = std::env::temp_dir().join(format!(
            "vokia-channels-test-{}-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos())
        ));
        let db = Db::open(&dir).expect("o banco de teste deveria abrir");
        TempDb { dir, db }
    }

    fn project(&self, name: &str) -> i64 {
        self.db
            .insert_project(&NewProject {
                name,
                symbol: "folder",
                description: "",
                path: &format!("C:/projetos/{name}"),
                origin_kind: "folder",
                origin_repo: None,
            })
            .expect("o projeto deveria ser criado")
            .id
    }

    fn channel(&self, project: i64, name: &str) -> Channel {
        self.db
            .create_channel(project, name, &members(&["axiom", "vexa"]))
            .expect("o canal deveria ser criado")
    }

    fn names(&self, project: i64) -> Vec<String> {
        self.db
            .list_channels(project)
            .unwrap()
            .into_iter()
            .map(|channel| channel.name)
            .collect()
    }

    fn channel_rows(&self) -> i64 {
        self.db
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM channels", [], |row| row.get(0))
            .unwrap()
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn members(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| id.to_string()).collect()
}

#[test]
fn canal_criado_volta_listado_com_membros_na_ordem_gravada() {
    let t = TempDb::open();
    let project = t.project("a");

    let created =
        t.db.create_channel(project, "front", &members(&["sentry", "axiom", "vexa"]))
            .unwrap();
    let listed = t.db.list_channels(project).unwrap();

    assert_eq!(created.project_id, project);
    assert_eq!(created.name, "front");
    assert_eq!(created.members, members(&["sentry", "axiom", "vexa"]));
    assert!(created.created_at > 0);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
    assert_eq!(listed[0].members, members(&["sentry", "axiom", "vexa"]));
}

#[test]
fn canais_listam_na_ordem_de_criacao_e_so_do_projeto_pedido() {
    let t = TempDb::open();
    let (first, second) = (t.project("a"), t.project("b"));
    t.channel(first, "zeta");
    t.channel(second, "outro");
    t.channel(first, "alfa");

    assert_eq!(t.names(first), vec!["zeta", "alfa"]);
}

#[test]
fn nome_repetido_no_mesmo_projeto_falha_com_a_mensagem_do_nome() {
    let t = TempDb::open();
    let project = t.project("a");
    t.channel(project, "front");

    let error =
        t.db.create_channel(project, "front", &members(&["kairo", "sentry"]))
            .unwrap_err();

    assert_eq!(
        error.to_string(),
        "Já existe um canal #front neste projeto."
    );
    assert_eq!(t.names(project), vec!["front"]);
}

#[test]
fn mesmo_nome_em_outro_projeto_e_aceito() {
    let t = TempDb::open();
    let (first, second) = (t.project("a"), t.project("b"));
    t.channel(first, "front");

    let created = t.channel(second, "front");

    assert_eq!(created.project_id, second);
}

#[test]
fn criar_em_projeto_inexistente_falha_sem_gravar() {
    let t = TempDb::open();

    let error =
        t.db.create_channel(999, "front", &members(&["axiom", "vexa"]))
            .unwrap_err();

    assert_eq!(error.to_string(), "O projeto 999 não existe.");
    assert_eq!(t.channel_rows(), 0);
}

#[test]
fn editar_troca_nome_e_membros_sem_mexer_nos_outros_canais() {
    let t = TempDb::open();
    let project = t.project("a");
    let target = t.channel(project, "um");
    t.channel(project, "dois");

    let edited =
        t.db.update_channel(project, target.id, "novo", &members(&["vexa", "nox", "axiom"]))
            .unwrap();

    assert_eq!(edited.id, target.id);
    assert_eq!(edited.name, "novo");
    assert_eq!(edited.members, members(&["vexa", "nox", "axiom"]));
    assert_eq!(t.names(project), vec!["novo", "dois"]);
}

#[test]
fn editar_para_nome_de_outro_canal_falha_e_preserva_o_original() {
    let t = TempDb::open();
    let project = t.project("a");
    let target = t.channel(project, "um");
    t.channel(project, "dois");

    let error = t.db.update_channel(project, target.id, "dois", &members(&["axiom"])).unwrap_err();

    assert!(matches!(error, DbError::ChannelNameTaken(_)));
    assert_eq!(t.names(project), vec!["um", "dois"]);
}

#[test]
fn editar_canal_de_outro_projeto_ou_inexistente_falha() {
    let t = TempDb::open();
    let (owner, other) = (t.project("a"), t.project("b"));
    let channel = t.channel(owner, "front");

    let foreign = t.db.update_channel(other, channel.id, "x1", &members(&["axiom"])).unwrap_err();
    let missing = t.db.update_channel(owner, 42, "x1", &members(&["axiom"])).unwrap_err();

    assert_eq!(foreign.to_string(), "Canal não encontrado.");
    assert_eq!(missing.to_string(), "Canal não encontrado.");
    assert_eq!(t.names(owner), vec!["front"]);
}

#[test]
fn excluir_remove_so_o_canal_pedido() {
    let t = TempDb::open();
    let project = t.project("a");
    let doomed = t.channel(project, "um");
    t.channel(project, "dois");

    t.db.delete_channel(project, doomed.id).unwrap();

    assert_eq!(t.names(project), vec!["dois"]);
}

#[test]
fn excluir_canal_de_outro_projeto_falha_e_preserva_o_canal() {
    let t = TempDb::open();
    let (owner, other) = (t.project("a"), t.project("b"));
    let channel = t.channel(owner, "front");

    let error = t.db.delete_channel(other, channel.id).unwrap_err();

    assert_eq!(error.to_string(), "Canal não encontrado.");
    assert_eq!(t.names(owner), vec!["front"]);
}

#[test]
fn excluir_canal_inexistente_falha() {
    let t = TempDb::open();
    let project = t.project("a");

    let error = t.db.delete_channel(project, 42).unwrap_err();

    assert_eq!(error.to_string(), "Canal não encontrado.");
}

#[test]
fn excluir_o_projeto_apaga_seus_canais_e_poupa_os_de_outro_projeto() {
    let t = TempDb::open();
    let (gone, kept) = (t.project("a"), t.project("b"));
    t.channel(gone, "front");
    t.channel(kept, "front");

    t.db.delete_project(gone).unwrap();

    assert_eq!(t.channel_rows(), 1);
    assert_eq!(t.names(kept), vec!["front"]);
}

#[test]
fn apagar_tudo_remove_todos_os_canais() {
    let t = TempDb::open();
    let (first, second) = (t.project("a"), t.project("b"));
    t.channel(first, "front");
    t.channel(second, "back");

    t.db.erase_all().unwrap();

    assert_eq!(t.channel_rows(), 0);
}
