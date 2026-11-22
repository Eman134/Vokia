use super::*;

#[test]
fn cls1_classifica_saudacao_pergunta_e_pedidos_de_trabalho() {
    for request in ["Oi", "Obrigado", "O que esse projeto faz?"] {
        assert_eq!(classify_request(request), TaskKind::Conversa, "{request}");
    }
    for request in [
        "Crie um card de login",
        "/plano revisar o fluxo",
        "Implemente o filtro",
        "Testar a conexão",
        "Corrigir bug no login",
    ] {
        assert_eq!(classify_request(request), TaskKind::Demanda, "{request}");
    }
}
