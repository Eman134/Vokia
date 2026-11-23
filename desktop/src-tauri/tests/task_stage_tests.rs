use super::*;
use crate::db::Layer;

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
