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
                who: None,
                after: None,
            },
            PlanLot {
                n: 2,
                title: "Tela".to_string(),
                meta: "Tela funcionando".to_string(),
                tasks: Vec::new(),
                who: None,
                after: None,
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

#[test]
fn roteiro_nao_vai_para_o_resumo() {
    let draft = parse_proposal(
        "Resumo limpo.\nSegunda linha do resumo.\n#plano-pronto\n\
         #roteiro 1|quem:ember~modelo:sonnet~ctx:Contexto~arq:a.ts\n\
         #lote 1|Base|Meta\n\
         #roteiro 2|quem:kairo~modelo:haiku~ctx:Segundo~arq:b.ts",
    );

    assert_eq!(draft.summary, "Resumo limpo.\nSegunda linha do resumo.");
    assert_eq!(draft.lots.len(), 1);
}

#[test]
fn roteiro_antes_do_lote_correspondente_nao_afeta_o_lote_nem_o_resumo() {
    let draft = parse_proposal(
        "Resumo inicial.\n#plano-pronto\n\
         #roteiro 1|quem:ember~modelo:sonnet~ctx:Contexto~arq:a.ts~passo:P1~passo:P2~pronto:OK~nao:Nao\n\
         #lote 1|Base|Meta|Tarefa 1; Tarefa 2",
    );

    assert_eq!(draft.summary, "Resumo inicial.");
    assert_eq!(draft.lots.len(), 1);
    assert_eq!(draft.lots[0].n, 1);
    assert_eq!(draft.lots[0].title, "Base");
    assert_eq!(draft.lots[0].meta, "Meta");
    assert_eq!(draft.lots[0].tasks, ["Tarefa 1", "Tarefa 2"]);
}

#[test]
fn roteiro_malformado_e_ignorado_e_nao_polui_resumo_nem_quebra_parser() {
    let draft = parse_proposal(
        "Resumo intacto.\n#plano-pronto\n\
         #roteiro\n\
         #roteiro   \n\
         #roteiro |\n\
         #roteiro sem-numero|invalido\n\
         #roteiro 1|chave-sem-dois-pontos\n\
         #roteiro 2|desconhecido:valor\n\
         #lote 1|Titulo|Meta\n\
         #lote 2|Outro|Meta2",
    );

    assert_eq!(draft.summary, "Resumo intacto.");
    assert_eq!(draft.lots.len(), 2);
    assert_eq!(draft.lots[0].title, "Titulo");
    assert_eq!(draft.lots[1].title, "Outro");
}

#[test]
fn proposta_antiga_sem_roteiro_mantem_regressao_exata() {
    let draft = parse_proposal(
        "Resumo legado do plano.\nSegunda linha do resumo.\n#plano-pronto\n\
         #estimativa ~3 dias\n\
         #lote 1|Backend|API pronta|Endpoint A; Endpoint B\n\
         #lote 2|Frontend|Interface pronta|Tela 1\n\
         #risco Dependência externa\n\
         #etiquetas backend, frontend",
    );

    assert_eq!(
        draft.summary,
        "Resumo legado do plano.\nSegunda linha do resumo."
    );
    assert_eq!(draft.estimate, "~3 dias");
    assert_eq!(draft.lots.len(), 2);
    assert_eq!(draft.lots[0].n, 1);
    assert_eq!(draft.lots[0].title, "Backend");
    assert_eq!(draft.lots[0].meta, "API pronta");
    assert_eq!(draft.lots[0].tasks, ["Endpoint A", "Endpoint B"]);
    assert_eq!(draft.lots[1].n, 2);
    assert_eq!(draft.lots[1].title, "Frontend");
    assert_eq!(draft.lots[1].meta, "Interface pronta");
    assert_eq!(draft.lots[1].tasks, ["Tela 1"]);
    assert_eq!(draft.risks, ["Dependência externa"]);
    assert_eq!(draft.tags, ["backend", "frontend"]);
}

#[test]
fn apos_define_dependencias_e_sem_apos_vale_o_lote_anterior() {
    let draft = parse_proposal(
        "#lote 1|A|a|x
#lote 2|B|b|x
#lote 3|C|c|x
#lote 4|D|d|x
         #roteiro 1|quem:kairo~apos:0
#roteiro 2|quem:vexa~apos:1
#roteiro 3|quem:kairo~apos:1
         #roteiro 4|quem:sentry~apos:2, 3",
    );

    assert_eq!(
        draft.dependencies(),
        [(1, vec![]), (2, vec![1]), (3, vec![1]), (4, vec![2, 3])]
    );
    assert_eq!(draft.dependency_refusal(), None);
}

#[test]
fn proposta_sem_apos_segue_em_sequencia() {
    let draft = parse_proposal("#lote 1|A|a|x
#lote 2|B|b|x
#lote 3|C|c|x");

    assert_eq!(draft.dependencies(), [(1, vec![]), (2, vec![1]), (3, vec![2])]);
}

#[test]
fn apos_para_lote_que_nao_vem_antes_e_recusado() {
    for after in ["2", "3", "9"] {
        let draft = parse_proposal(&format!(
            "#lote 1|A|a|x
#lote 2|B|b|x
#lote 3|C|c|x
#roteiro 2|quem:kairo~apos:{after}"
        ));

        assert!(draft.dependency_refusal().is_some(), "apos:{after}");
    }
}
