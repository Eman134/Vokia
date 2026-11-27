use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use tauri::Manager;

use super::*;
use crate::agents::AgentRegistry;
use crate::db::{DbState, NewProject};
use crate::mcp::support::build_app;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const OLD_MARKER_LINES: &str =
    "Texto.\n#card criar Fantasma|x\n#decisao pergunta|Posso?||\n$contagem = 3\n@Vexa: revise isso";

struct World {
    app: tauri::App,
    dir: PathBuf,
    project_id: i64,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn world() -> World {
    let dir = std::env::temp_dir().join(format!(
        "vokia-text-fallback-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    let app = build_app(&dir);
    let project_id = app
        .state::<DbState>()
        .db()
        .expect("banco de teste aberto")
        .insert_project(&NewProject {
            name: "a",
            symbol: "folder",
            description: "",
            path: "C:/projetos/a",
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("o projeto deveria ser criado")
        .id;
    World {
        app,
        dir,
        project_id,
    }
}

impl World {
    fn read(&self, with_tools: bool, speaker: &str, reply: &str) -> TextReply {
        let team = self
            .app
            .state::<AgentRegistry>()
            .team()
            .expect("o time padrão deveria carregar");
        let speaker = Agent::from_stored(speaker).expect("agente");
        let state = self.app.state::<AppState>();
        let context = TextContext {
            app: self.app.handle(),
            state: state.inner(),
            project_id: self.project_id,
            speaker: &speaker,
            item_id: None,
            team: &team,
        };
        read_reply(with_tools, &context, reply).expect("a leitura da resposta não deveria falhar")
    }

    fn cards(&self) -> Vec<crate::db::BacklogItem> {
        self.app
            .state::<DbState>()
            .db()
            .expect("banco de teste aberto")
            .list_backlog(self.project_id)
            .expect("backlog")
            .items
    }

    fn number(&self, name: &str) -> Option<i64> {
        let state = self.app.state::<AppState>();
        with_cache(state.inner(), self.project_id, |cache| {
            Ok(cache.number(name))
        })
        .expect("cache")
    }
}

#[test]
fn sem_mcp_card_criar_grava_o_card_do_agente_e_a_linha_some_do_texto() {
    let world = world();

    let reply = world.read(
        false,
        "axiom",
        "Registrando.\n#card criar Revisar login|Checar o token\nPronto.",
    );

    assert_eq!(reply.visible, "Registrando.\nPronto.");
    assert!(reply.rejected.is_empty(), "{:?}", reply.rejected);
    assert!(reply.cards.changed);
    assert_eq!(
        reply.cards.confirmations,
        ["Card T1 criado: Revisar login."]
    );
    let cards = world.cards();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].title, "Revisar login");
    assert_eq!(cards[0].origin, "agente");
}

#[test]
fn sem_mcp_card_mover_leva_o_card_do_proprio_agente_para_a_coluna() {
    let world = world();
    world.read(false, "axiom", "#card criar Um");

    let reply = world.read(false, "axiom", "#card mover T1|In progress");

    assert!(reply.rejected.is_empty(), "{:?}", reply.rejected);
    assert_eq!(
        reply.cards.confirmations,
        ["Card T1 movido para In progress."]
    );
    assert_eq!(world.cards()[0].column.as_deref(), Some("In progress"));
}

#[test]
fn sem_mcp_decisao_vira_decisao_para_subir_e_a_linha_some_do_texto() {
    let world = world();

    let reply = world.read(
        false,
        "axiom",
        "Preciso decidir.\n#decisao escolha|Qual caminho?|Com detalhe|A;B",
    );
    let decision = reply.decision.expect("a linha #decisao deveria ser lida");

    assert_eq!(decision.title, "Qual caminho?");
    assert_eq!(decision.detail, "Com detalhe");
    assert_eq!(decision.options, ["A", "B"]);
    assert_eq!(decision.visible, "Preciso decidir.");
    assert!(world.cards().is_empty());
}

#[test]
fn sem_mcp_variavel_de_texto_altera_o_cache_e_conta_como_avanco() {
    let world = world();

    let reply = world.read(false, "axiom", "Feito.\n$contagem = 3");

    assert_eq!(reply.visible, "Feito.");
    assert!(reply.rejected.is_empty(), "{:?}", reply.rejected);
    assert!(reply.applied.created_variable);
    assert_eq!(reply.applied.accepted.len(), 1);
    assert_eq!(world.number("contagem"), Some(3));
}

#[test]
fn sem_mcp_nome_do_colega_com_dois_pontos_delega_e_a_linha_some_do_texto() {
    let world = world();

    let reply = world.read(false, "axiom", "Preciso de revisão.\n@Vexa: revise o login");

    assert_eq!(reply.visible, "Preciso de revisão.");
    assert_eq!(
        reply.delegation,
        Some((
            Agent::from_stored("vexa").expect("agente"),
            "revise o login".to_string()
        ))
    );
}

#[test]
fn sem_mcp_uma_linha_invalida_recusa_a_resposta_inteira_e_nada_e_gravado() {
    let world = world();

    let reply = world.read(false, "axiom", "$ok = 1\n#card criar |sem título");

    assert_eq!(reply.rejected, ["o título do card não pode ser vazio"]);
    assert!(!reply.cards.changed);
    assert!(world.cards().is_empty());
}

#[test]
fn sem_mcp_marcador_de_variavel_mal_formado_e_recusado_sem_gravar_o_resto() {
    let world = world();

    let reply = world.read(false, "axiom", "$bom = 1\n$ruim");

    assert!(!reply.rejected.is_empty());
    assert_eq!(world.number("bom"), None);
    assert!(reply.applied.accepted.is_empty());
}

#[test]
fn com_mcp_so_a_linha_de_mencao_age_e_as_outras_aparecem_como_texto_sem_executar() {
    let world = world();

    let reply = world.read(true, "axiom", OLD_MARKER_LINES);

    assert_eq!(reply.delegation, Some((vexa(), "revise isso".to_string())));
    assert_eq!(
        reply.visible,
        "Texto.\n#card criar Fantasma|x\n#decisao pergunta|Posso?||\n$contagem = 3"
    );
    assert!(reply.decision.is_none());
    assert!(reply.rejected.is_empty());
    assert!(reply.applied.accepted.is_empty() && !reply.applied.created_variable);
    assert!(!reply.cards.changed && reply.cards.confirmations.is_empty());
    assert!(world.cards().is_empty());
    assert_eq!(world.number("contagem"), None);
}

fn vexa() -> Agent {
    Agent::from_stored("vexa").expect("agente")
}

#[test]
fn com_mcp_mencao_de_agente_real_delega_e_a_linha_sai_da_fala() {
    let world = world();

    let reply = world.read(true, "axiom", "Preciso de revisão.\n@Vexa: revise o login");

    assert_eq!(
        reply.delegation,
        Some((vexa(), "revise o login".to_string()))
    );
    assert_eq!(reply.visible, "Preciso de revisão.");
}

#[test]
fn com_mcp_so_a_mencao_nao_deixa_fala_do_agente() {
    let world = world();

    let reply = world.read(true, "axiom", "@Vexa: vexa oi");

    assert_eq!(reply.delegation, Some((vexa(), "vexa oi".to_string())));
    assert_eq!(reply.visible, "");
}

#[test]
fn com_mcp_mencao_de_quem_nao_existe_ou_de_si_mesmo_ou_sem_pedido_fica_texto() {
    let world = world();

    for text in [
        "@Fantasma: faça isso",
        "@Axiom: faça isso",
        "@Vexa:",
        "@Vexa sem dois pontos",
    ] {
        let reply = world.read(true, "axiom", text);

        assert!(reply.delegation.is_none(), "{text}");
        assert_eq!(reply.visible, text);
    }
}

#[test]
fn com_mcp_a_ultima_mencao_valida_vale() {
    let world = world();

    let reply = world.read(
        true,
        "axiom",
        "@Fantasma: x\n@Vexa: primeiro\n@Vexa: segundo",
    );

    assert_eq!(reply.delegation, Some((vexa(), "segundo".to_string())));
}

#[test]
fn sem_mcp_a_mencao_segue_delegando_e_a_linha_continua_saindo_do_texto() {
    let world = world();

    let reply = world.read(false, "axiom", "Preciso.\n@Vexa: revise");

    assert_eq!(reply.delegation, Some((vexa(), "revise".to_string())));
    assert_eq!(reply.visible, "Preciso.");
}
