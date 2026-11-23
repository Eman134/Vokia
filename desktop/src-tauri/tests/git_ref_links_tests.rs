use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::db::{BacklogItem, Db, DbError, GitRefLinkInput, Link, NewCard, NewProject};
use crate::projects::list_git_refs;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!(
            "vokia-{prefix}-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).expect("diretório temporário deveria ser criado");
        TempDir(dir)
    }

    fn open(&self) -> Db {
        Db::open(&self.0).expect("o banco de teste deveria abrir")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create_project_and_card(db: &Db) -> (i64, BacklogItem) {
    let project_id = db
        .insert_project(&NewProject {
            name: "vokia",
            symbol: "folder",
            description: "",
            path: "C:/projetos/vokia",
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("o projeto deveria ser criado")
        .id;
    let card = db
        .create_card(
            project_id,
            &NewCard {
                title: "Vincular commit ao card".to_string(),
                description: None,
                origin: "usuario".to_string(),
                links: vec![Link {
                    id: "spec-1".to_string(),
                    label: "Especificação".to_string(),
                    kind: None,
                    url: None,
                }],
            },
        )
        .expect("o card deveria ser criado");
    (project_id, card)
}

fn stored(db: &Db, project_id: i64, id: &str) -> BacklogItem {
    db.list_backlog(project_id)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("o card {id} deveria existir"))
}

#[test]
fn link_e_unlink_git_ref_preservam_links_antigos_e_evitam_duplicatas() {
    let dir = TempDir::new("git-links");
    let db = dir.open();
    let (project_id, card) = create_project_and_card(&db);

    let created = db
        .link_git_ref(
            project_id,
            &card.id,
            &GitRefLinkInput {
                kind: "commit".to_string(),
                ref_id: "abc1234".to_string(),
                label: Some("feat: add link".to_string()),
                url: Some("https://github.com/org/repo/commit/abc1234".to_string()),
            },
        )
        .expect("deveria vincular commit");

    assert_eq!(created.id, "commit:abc1234");
    assert_eq!(created.kind.as_deref(), Some("commit"));
    assert_eq!(created.label, "feat: add link");

    let dup = db.link_git_ref(
        project_id,
        &card.id,
        &GitRefLinkInput {
            kind: "commit".to_string(),
            ref_id: "abc1234".to_string(),
            label: None,
            url: None,
        },
    );
    assert!(matches!(dup, Err(DbError::DuplicateGitRef(_))));

    let item = stored(&db, project_id, &card.id);
    assert_eq!(item.links.len(), 2);
    assert_eq!(item.links[0].id, "spec-1");
    assert_eq!(item.links[0].kind, None);

    db.unlink_git_ref(project_id, &card.id, "commit:abc1234")
        .expect("deveria desvincular commit");

    let after = stored(&db, project_id, &card.id);
    assert_eq!(after.links.len(), 1);
    assert_eq!(after.links[0].id, "spec-1");
}

#[test]
fn link_git_ref_recusa_tipo_invalido_e_referencia_vazia() {
    let dir = TempDir::new("git-links-invalid");
    let db = dir.open();
    let (project_id, card) = create_project_and_card(&db);

    let bad_kind = db.link_git_ref(
        project_id,
        &card.id,
        &GitRefLinkInput {
            kind: "tag".to_string(),
            ref_id: "v1.0".to_string(),
            label: None,
            url: None,
        },
    );
    assert!(matches!(bad_kind, Err(DbError::InvalidGitRefKind(_))));

    let empty_ref = db.link_git_ref(
        project_id,
        &card.id,
        &GitRefLinkInput {
            kind: "branch".to_string(),
            ref_id: "   ".to_string(),
            label: None,
            url: None,
        },
    );
    assert!(matches!(empty_ref, Err(DbError::EmptyGitRef)));

    let missing_unlink = db.unlink_git_ref(project_id, &card.id, "branch:nao-existe");
    assert!(matches!(missing_unlink, Err(DbError::GitRefNotFound(_))));
}

#[test]
fn link_sem_kind_e_url_mantem_compatibilidade_json() {
    let raw = r#"{"id":"doc-1","label":"Arquitetura"}"#;
    let parsed: Link = serde_json::from_str(raw).expect("deveria desserializar Link antigo");
    assert_eq!(parsed.id, "doc-1");
    assert_eq!(parsed.label, "Arquitetura");
    assert_eq!(parsed.kind, None);
    assert_eq!(parsed.url, None);

    let serialized = serde_json::to_string(&parsed).expect("deveria serializar");
    assert_eq!(serialized, raw);
}

#[tokio::test]
async fn list_git_refs_retorna_branches_e_commits_recentes() {
    let dir = TempDir::new("git-refs-repo");
    let repo = &dir.0;

    let run_git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo)
            .status()
            .expect("git deveria executar");
        assert!(status.success(), "git {args:?} falhou");
    };

    run_git(&["init", "-b", "main"]);
    run_git(&["config", "user.name", "Test"]);
    run_git(&["config", "user.email", "test@example.com"]);
    fs::write(repo.join("README.md"), "# test\n").unwrap();
    run_git(&["add", "README.md"]);
    run_git(&["commit", "-m", "initial commit"]);
    run_git(&["checkout", "-b", "feat/item-85"]);

    let refs = list_git_refs(repo.to_str().unwrap(), Some(10))
        .await
        .expect("list_git_refs deveria funcionar");

    assert!(!refs.commits.is_empty());
    assert_eq!(refs.commits[0].subject, "initial commit");
    assert!(refs.branches.iter().any(|b| b.name == "feat/item-85" && b.current));
    assert!(refs.branches.iter().any(|b| b.name == "main"));
}
