use super::*;

const MULTIBYTE: char = 'é';

fn run(agent: &str, item_id: Option<&str>) -> InterruptedRun {
    InterruptedRun {
        id: 7,
        project_id: 1,
        agent: agent.to_string(),
        request: "Implementar o login".to_string(),
        run_id: None,
        item_id: item_id.map(str::to_string),
        started_at: 1_000,
        leader: false,
    }
}

fn handoff(from: &str, to: &str, item_id: Option<&str>) -> RunHandoff {
    RunHandoff {
        from_agent: from.to_string(),
        to_agent: to.to_string(),
        kind: "implementar".to_string(),
        item_id: item_id.map(str::to_string),
    }
}

fn message(from: &str, to: &str, text: &str, activity: Option<&str>) -> RunMessage {
    RunMessage {
        from_agent: from.to_string(),
        to_agent: to.to_string(),
        text: text.to_string(),
        activity: activity.map(|json| serde_json::from_str(json).expect("atividade de teste válida")),
    }
}

fn lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

#[test]
fn com_delegacoes_e_ember_o_prompt_traz_delegacoes_e_pede_coordenacao_no_fim() {
    let progress = RunProgress {
        handoffs: vec![handoff("vexa", "hefesto", Some("item-1")), handoff("vexa", "axiom", None)],
        messages: Vec::new(),
    };

    let text = continuation(&run("vexa", Some("item-1")), &progress, true);

    assert_eq!(
        lines(&text),
        [
            RESUME_HEADING,
            "Pedido original: Implementar o login",
            "Agente que atendia: vexa",
            "Card: item-1",
            "Delegações já feitas:",
            "- vexa → hefesto (implementar de item-1)",
            "- vexa → axiom (implementar)",
            EMBER_COORDINATION,
        ]
    );
}

#[test]
fn ember_original_com_delegacoes_nao_recebe_instrucao_de_coordenacao() {
    let progress = RunProgress {
        handoffs: vec![handoff("ember", "vexa", None)],
        messages: Vec::new(),
    };

    let text = continuation(&run("ember", None), &progress, false);

    assert!(text.contains(HANDOFFS_HEADING), "{text}");
    assert!(!text.contains(EMBER_COORDINATION), "{text}");
    assert!(!text.contains(CARD_LABEL), "sem card não há linha de card: {text}");
}

#[test]
fn sem_nada_registrado_o_prompt_manda_comecar_pelo_pedido_original() {
    let empty = RunProgress {
        handoffs: Vec::new(),
        messages: Vec::new(),
    };

    let text = continuation(&run("vexa", Some("item-1")), &empty, false);

    assert_eq!(lines(&text).last().copied(), Some(NOTHING_RECORDED));
    assert!(!text.contains(HANDOFFS_HEADING));
    assert!(!text.contains(MESSAGES_HEADING));
}

#[test]
fn mensagens_entram_em_ordem_com_as_ferramentas_usadas() {
    let progress = RunProgress {
        handoffs: Vec::new(),
        messages: vec![
            message("vexa", "user", "Comecei pela tabela", None),
            message(
                "vexa",
                "user",
                "Tabela criada",
                Some(r#"{"tools":[{"callId":"1","name":"Edit","category":"edit","target":"src/db.rs","status":"ok"},{"callId":"2","name":"Bash","category":"command","target":"","status":"failed"}]}"#),
            ),
        ],
    };

    let text = continuation(&run("vexa", None), &progress, false);

    assert_eq!(
        lines(&text)[3..],
        [
            "Mensagens já trocadas (mais antigas primeiro):",
            "- vexa → user: Comecei pela tabela",
            "- vexa → user: Tabela criada",
            "  ferramentas: Edit src/db.rs; Bash",
        ]
    );
    assert!(!text.contains(NOTHING_RECORDED));
}

#[test]
fn texto_longo_e_cortado_em_1500_caracteres_sem_quebrar_multibyte() {
    let long: String = std::iter::repeat_n(MULTIBYTE, PROGRESS_TEXT_MAX_CHARS + 100).collect();
    let exact: String = std::iter::repeat_n(MULTIBYTE, PROGRESS_TEXT_MAX_CHARS).collect();

    let cut = message_lines(&message("vexa", "user", &long, None));
    let kept = message_lines(&message("vexa", "user", &exact, None));

    let prefix = "- vexa → user: ";
    let body = cut[0].strip_prefix(prefix).expect("linha da mensagem");
    assert_eq!(body.chars().count(), PROGRESS_TEXT_MAX_CHARS);
    assert!(body.chars().all(|c| c == MULTIBYTE));
    assert_eq!(kept[0], format!("{prefix}{exact}"));
}
