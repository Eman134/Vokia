use super::{parse_proposal, PlanLot};

#[test]
fn proposta_completa_vira_resumo_estimativa_lotes_riscos_e_etiquetas() {
    let draft = parse_proposal(
        "Primeira linha.\nSegunda linha.\n#plano-pronto\n#estimativa 2 dias\n\
         #lote 1|Base|Base pronta|Criar tabela; Criar API\n#lote 2|Tela|Tela funcionando\n\
         #risco Prazo\n#risco API instável\n#etiquetas auth, ui",
    );

    assert_eq!(draft.summary, "Primeira linha.\nSegunda linha.");
    assert_eq!(draft.estimate, "2 dias");
    assert_eq!(
        draft.lots,
        [
            PlanLot {
                n: 1,
                title: "Base".to_string(),
                meta: "Base pronta".to_string(),
                tasks: vec!["Criar tabela".to_string(), "Criar API".to_string()],
            },
            PlanLot {
                n: 2,
                title: "Tela".to_string(),
                meta: "Tela funcionando".to_string(),
                tasks: Vec::new(),
            },
        ]
    );
    assert_eq!(draft.risks, ["Prazo", "API instável"]);
    assert_eq!(draft.tags, ["auth", "ui"]);
}

#[test]
fn texto_vazio_ou_so_com_o_marcador_de_pronto_nao_gera_nada() {
    for text in ["", "   \n  ", "#plano-pronto"] {
        let draft = parse_proposal(text);

        assert_eq!(draft.summary, "", "{text:?}");
        assert!(draft.lots.is_empty(), "{text:?}");
        assert!(draft.risks.is_empty() && draft.tags.is_empty(), "{text:?}");
        assert_eq!(draft.estimate, "", "{text:?}");
    }
}

#[test]
fn lote_sem_titulo_e_descartado_e_nao_consome_numero() {
    let draft = parse_proposal("#lote 1||sem título|tarefa\n#lote |   |meta\n#lote 3|Válido|Meta");

    assert_eq!(draft.lots.len(), 1);
    assert_eq!(draft.lots[0].n, 1);
    assert_eq!(draft.lots[0].title, "Válido");
}

#[test]
fn lote_sem_numero_inicial_usa_a_posicao_e_aceita_so_o_titulo() {
    let draft = parse_proposal("#lote Só título\n#lote Base|Meta B|A;;  ; B ;");

    assert_eq!(draft.lots[0].n, 1);
    assert_eq!(draft.lots[0].title, "Só título");
    assert_eq!(draft.lots[0].meta, "");
    assert!(draft.lots[0].tasks.is_empty());
    assert_eq!(draft.lots[1].n, 2);
    assert_eq!(draft.lots[1].tasks, ["A", "B"]);
}

#[test]
fn numero_do_lote_vem_da_ordem_e_nao_do_texto() {
    let draft = parse_proposal("#lote 7|Primeiro|M1\n#lote 7|Segundo|M2");

    assert_eq!(draft.lots.iter().map(|lot| lot.n).collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn risco_vazio_e_etiquetas_em_branco_sao_ignorados() {
    let draft = parse_proposal("#risco   \n#etiquetas  , ,a,  ,b ");

    assert!(draft.risks.is_empty());
    assert_eq!(draft.tags, ["a", "b"]);
}

#[test]
fn titulo_que_comeca_com_digitos_e_texto_nao_e_tratado_como_numero_de_lote() {
    let draft = parse_proposal("#lote 2fa|Autenticação|Meta");

    assert_eq!(draft.lots[0].title, "2fa");
    assert_eq!(draft.lots[0].meta, "Autenticação");
}
