use super::*;

#[test]
fn pedido_sem_canal_chega_ao_agente_identico_byte_a_byte() {
    assert_eq!(
        with_channel_note("faça isso\ne aquilo", None),
        "faça isso\ne aquilo"
    );
}

#[test]
fn nota_do_canal_entra_uma_vez_no_fim_do_pedido() {
    let found = FoundChannel {
        id: 1,
        note: channel_note("front", &["Davi".to_string(), "Nina".to_string()]),
    };

    let request = with_channel_note("faça isso", Some(&found));

    assert_eq!(
        request,
        "faça isso\nMensagem enviada no canal #front (membros: Davi, Nina)."
    );
}
