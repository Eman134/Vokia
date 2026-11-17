use super::*;

fn agent(id: &str) -> Agent {
    Agent::from_stored(id).expect("agente")
}

fn definition() -> TaskDefinition {
    TaskDefinition {
        nova_tarefa: true,
        objetivo: "teste".to_string(),
        criterion: String::new(),
        verify: false,
        participants: vec![agent("iara"), agent("davi")],
        alternate: false,
        variables: Vec::new(),
        stop: None,
    }
}

#[test]
fn delegacao_abre_turno_do_helper_com_o_requisitante_na_fila() {
    let definition = definition();
    let iara = Turn::new(agent("iara"), "pedido", "pedido", Vec::new());
    let delegation = (agent("davi"), "davi oi".to_string());

    let next = next_turn(&definition, &iara, "Oi", Some(&delegation)).expect("turno do helper");

    assert!(next.speaker == agent("davi"));
    assert_eq!(next.received, "davi oi");
    assert!(next.requesters.len() == 1 && next.requesters[0] == agent("iara"));
}

#[test]
fn fala_com_delegacao_vai_a_quem_pediu_e_nao_ao_helper() {
    let definition = definition();
    let iara = Turn::new(agent("iara"), "pedido", "pedido", Vec::new());
    let delegation = (agent("davi"), "davi oi".to_string());
    let next = next_turn(&definition, &iara, "Oi", Some(&delegation));

    assert!(speech_recipient(&definition, &iara, next.as_ref()) == Speaker::User);
}

#[test]
fn resposta_do_helper_fica_na_linha_dele_e_nao_vira_turno_do_requisitante() {
    let definition = definition();
    let davi = Turn::new(agent("davi"), "davi oi", "davi oi", vec![agent("iara")]);

    let next = next_turn(&definition, &davi, "Oi! Em que posso ajudar?", None);

    assert!(next.is_none());
    assert!(speech_recipient(&definition, &davi, None) == Speaker::from(agent("iara")));
}

#[test]
fn sem_delegacao_nem_requisitante_a_fala_vai_ao_usuario_e_a_conversa_termina() {
    let definition = definition();
    let iara = Turn::new(agent("iara"), "pedido", "pedido", Vec::new());

    assert!(next_turn(&definition, &iara, "Oi", None).is_none());
    assert!(speech_recipient(&definition, &iara, None) == Speaker::User);
}
