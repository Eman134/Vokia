use super::*;
use crate::db::{Layer, OrchestrationSnapshot};

fn layer(id: &str, name: &str, position: i64, kind: &str) -> Layer {
    Layer {
        id: id.to_string(),
        name: name.to_string(),
        description: String::new(),
        position,
        kind: kind.to_string(),
        instructions: String::new(),
    }
}

// Liderança (planejar), Implementação, Revisão e Entrega, nessa ordem no mapa.
fn layers() -> Vec<Layer> {
    vec![
        layer("lideranca", "Liderança", 0, "planejar"),
        layer("implementacao", "Implementação", 1, "implementar"),
        layer("revisao", "Revisão", 2, "revisar"),
        layer("entrega", "Entrega", 3, "implementar"),
    ]
}

#[test]
fn julgamento_le_a_ultima_linha_valida_e_tira_a_linha_da_fala() {
    let reply = "Vou avaliar.\n#fluxo completo|primeira\nTexto final\n#fluxo reduzido|tarefa pequena";

    let judgment = parse_judgment(reply).expect("deveria achar o julgamento");

    assert_eq!(judgment.mode, FLOW_REDUCED);
    assert_eq!(judgment.motive, "tarefa pequena");
    // Toda linha #fluxo válida sai da fala, não só a que vale.
    assert_eq!(without_judgment_lines(reply), "Vou avaliar.\nTexto final");
}

#[test]
fn julgamento_com_modo_desconhecido_ou_sem_espaco_vira_texto_comum() {
    assert_eq!(parse_judgment("#fluxo simples|x"), None);
    assert_eq!(parse_judgment("#fluxos completo|x"), None);
    assert_eq!(
        parse_judgment("#fluxo completo").map(|judgment| judgment.motive),
        Some(String::new())
    );
}

#[test]
fn passagem_para_tras_ou_entre_camadas_vizinhas_nao_pula_nada() {
    let layers = layers();

    assert!(skipped_layers(&layers, "revisao", "implementacao").is_empty());
    assert!(skipped_layers(&layers, "implementacao", "revisao").is_empty());
    assert!(skipped_layers(&layers, "lideranca", "camada-inexistente").is_empty());
}

#[test]
fn completo_recusa_pular_qualquer_camada_e_reduzido_so_pula_revisao() {
    let layers = layers();

    // Liderança -> Entrega pula Implementação e Revisão: no reduzido a Implementação é obrigatória.
    assert!(skip_motive(&layers, Some("lideranca"), Some("entrega"), Some(FLOW_REDUCED))
        .is_some_and(|motive| motive.contains("Implementação")));
    // Implementação -> Entrega pula só a Revisão: passa no reduzido, recusa no completo.
    assert_eq!(
        skip_motive(&layers, Some("implementacao"), Some("entrega"), Some(FLOW_REDUCED)),
        None
    );
    assert!(skip_motive(&layers, Some("implementacao"), Some("entrega"), Some(FLOW_FULL))
        .is_some_and(|motive| motive.contains("Revisão") && motive.contains("completo")));
}

#[test]
fn sem_julgamento_gravado_a_task_vale_como_completa() {
    let layers = layers();

    assert!(skip_motive(&layers, Some("implementacao"), Some("entrega"), None).is_some());
    assert_eq!(skip_motive(&layers, Some("implementacao"), Some("revisao"), None), None);
}

#[test]
fn sem_camada_de_origem_ou_destino_nao_ha_recusa_por_pulo() {
    let layers = layers();

    assert_eq!(skip_motive(&layers, None, Some("entrega"), None), None);
    assert_eq!(skip_motive(&layers, Some("lideranca"), None, None), None);
}

#[test]
fn parada_reenvia_enquanto_houver_tentativas_e_depois_escala() {
    assert_eq!(stall_action(0, 2), StallAction::Resend);
    assert_eq!(stall_action(1, 2), StallAction::Resend);
    assert_eq!(stall_action(2, 2), StallAction::Escalate);
}

fn team() -> OrchTeam {
    OrchTeam {
        id: "time".to_string(),
        name: "Core".to_string(),
        position: 0,
        leader_agent: Some("axiom".to_string()),
        instructions: String::new(),
        final_validation: String::new(),
        start_x: None,
        start_y: None,
        end_x: None,
        end_y: None,
        judge_full_flow: false,
    }
}

fn placement(agent: &str, layer: &str) -> crate::db::Placement {
    crate::db::Placement {
        agent_id: agent.to_string(),
        team_id: "time".to_string(),
        layer_id: layer.to_string(),
        instructions: String::new(),
        max_instances: 1,
        pos_x: 0.0,
        pos_y: 0.0,
        criteria: String::new(),
        max_worktrees: None,
    }
}

fn snapshot(placements: Vec<crate::db::Placement>) -> OrchestrationSnapshot {
    OrchestrationSnapshot {
        settings: crate::db::OrchestrationSettings {
            enforcement: String::new(),
            leader_agent: "axiom".to_string(),
            step_mode: String::new(),
            human_gate: String::new(),
            max_parallel: 1,
            pair_turn_limit: 1,
            task_turn_limit: 1,
            worktree_policy: String::new(),
        },
        layers: layers(),
        teams: vec![team()],
        placements,
        edges: Vec::new(),
        gateways: Vec::new(),
        task_stages: Vec::new(),
    }
}

#[test]
fn roteiro_recusa_agente_fora_do_time() {
    let snapshot = snapshot(vec![placement("kairo", "implementacao")]);
    let refusal = plan_script_refusal(&snapshot, &team(), &[(1, Some("kairo")), (2, Some("vexa"))]);
    assert_eq!(
        refusal.as_deref(),
        Some("A etapa 2 é de vexa, que não está no time Core.")
    );
}

#[test]
fn roteiro_aceita_etapa_inicial_de_qualquer_membro_e_camada_pulada() {
    let snapshot = snapshot(vec![
        placement("kairo", "implementacao"),
        placement("vexa", "entrega"),
    ]);
    let lots = [(1, Some("vexa")), (2, Some("kairo")), (3, None)];
    assert_eq!(plan_script_refusal(&snapshot, &team(), &lots), None);
}

fn prompt_text(snapshot: &OrchestrationSnapshot, agent: &str) -> String {
    let dir = std::env::temp_dir().join(format!("vokia-judgment-prompt-{}-{agent}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cast = crate::agents::AgentRegistry::load(&dir).team().expect("time padrão");
    let agent = crate::agents::Agent::from_stored(agent).expect("agente");
    let prompt = crate::orchestration::snapshot::agent_prompt(Some(snapshot), &cast, &agent);
    let _ = std::fs::remove_dir_all(&dir);
    prompt.layer.and_then(|layer| layer.map_rules).unwrap_or_default()
}

fn judged(mut snapshot: OrchestrationSnapshot, enabled: bool) -> OrchestrationSnapshot {
    snapshot.teams[0].judge_full_flow = enabled;
    snapshot
}

#[test]
fn lider_com_julgamento_ligado_recebe_a_regra_do_fluxo() {
    let placements = vec![placement("axiom", "lideranca"), placement("kairo", "implementacao")];

    assert!(prompt_text(&judged(snapshot(placements.clone()), true), "axiom").contains(JUDGMENT_RULE));
    assert!(!prompt_text(&judged(snapshot(placements.clone()), false), "axiom").contains(JUDGMENT_RULE));
    // Só o líder julga: o membro do mesmo time não recebe a regra.
    assert!(!prompt_text(&judged(snapshot(placements), true), "kairo").contains(JUDGMENT_RULE));
}
