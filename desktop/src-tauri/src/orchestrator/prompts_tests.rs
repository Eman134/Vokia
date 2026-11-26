use super::*;
use crate::agents::{AgentProfile, AgentRegistry, Team};
use crate::mcp::ToolScope;

fn with_test_context(check: impl FnOnce(&Team, &AgentProfile)) {
    let dir = std::env::temp_dir().join(format!(
        "vokia-prompts-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let team = AgentRegistry::load(&dir)
        .team()
        .expect("team padrão");
    let profile = team.profiles().first().expect("profile").clone();
    check(&team, &profile);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn regra_de_administracao_contem_orientacao_mcp_para_app_commands() {
    assert!(ADMIN_TOOLS_RULE.contains("app_commands_list"));
    assert!(ADMIN_TOOLS_RULE.contains("app_command_call"));
    assert!(ADMIN_TOOLS_RULE.contains("opcionalmente filtrando domain"));
    assert!(!TOOLS_RULE.contains("app_command"));
}

#[test]
fn tools_rule_vokia_cli_contem_orientacao_cli_para_commands() {
    assert!(TOOLS_RULE_VOKIA_CLI.contains("vokia-cli commands [--domain <dom>]"));
    assert!(TOOLS_RULE_VOKIA_CLI.contains("vokia-cli cmd <comando> --json <obj>"));
}

#[test]
fn maestro_role_inclui_orientacoes_conforme_tool_mode() {
    with_test_context(|team, _| {
        let choices = ProjectChoices {
            pinned: Some("projeto-a".to_string()),
            labels: vec!["projeto-a".to_string()],
        };

        // Modo MCP
        let prompt_mcp = maestro_role(
            "Base Ember.",
            &[],
            &choices,
                        None,
            team,
            &[],
            ToolPromptMode::Mcp,
            false,
        );
        assert!(prompt_mcp.contains(MAESTRO_APP_COMMANDS_MCP));
        assert!(prompt_mcp.contains("app_commands_list"));
        assert!(prompt_mcp.contains("app_command_call"));
        assert!(!prompt_mcp.contains(MAESTRO_APP_COMMANDS_VOKIA_CLI));

        // Modo VokiaCli
        let prompt_cli = maestro_role(
            "Base Ember.",
            &[],
            &choices,
                        None,
            team,
            &[],
            ToolPromptMode::VokiaCli,
            false,
        );
        assert!(prompt_cli.contains(MAESTRO_APP_COMMANDS_VOKIA_CLI));
        assert!(prompt_cli.contains("vokia-cli commands [--domain <dom>]"));
        assert!(prompt_cli.contains("vokia-cli cmd <comando> --json <obj>"));
        assert!(!prompt_cli.contains(MAESTRO_APP_COMMANDS_MCP));

        // Modo None
        let prompt_none = maestro_role(
            "Base Ember.",
            &[],
            &choices,
                        None,
            team,
            &[],
            ToolPromptMode::None,
            false,
        );
        assert!(!prompt_none.contains(MAESTRO_APP_COMMANDS_MCP));
        assert!(!prompt_none.contains(MAESTRO_APP_COMMANDS_VOKIA_CLI));
    });
}

#[test]
fn tok3_catalogo_cli_e_step_prompt_sem_variaveis_ficam_compactos() {
    with_test_context(|team, _| {
        let choices = ProjectChoices {
            pinned: Some("projeto-a".to_string()),
            labels: vec!["projeto-a".to_string()],
        };
        let mcp = maestro_role(
            "Base Ember.",
            &[serde_json::json!({"name": "dummy_tool", "description": "desc"})],
            &choices,
                        None,
            team,
            &[],
            ToolPromptMode::Mcp,
            false,
        );
        assert!(!mcp.contains("Comandos disponíveis:"));
        assert!(!mcp.contains("dummy_tool"));
        assert!(mcp.contains("app_commands_list"));

        let cli = maestro_role(
            "Base Ember.",
            &[],
            &choices,
                        None,
            team,
            &[],
            ToolPromptMode::VokiaCli,
            false,
        );
        assert!(cli.contains("Comandos disponíveis:"));
        assert!(cli.contains("list_backlog"));
        assert!(cli.contains("vokia-cli tools"));
        assert!(cli.chars().count() < 2000);
    });

    let agent = Agent::ember();
    let cache = crate::task::Cache::default();
    let plain = crate::task::TaskDefinition {
        kind: crate::task::TaskKind::Conversa,
        nova_tarefa: true,
        objetivo: "Oi".to_string(),
        criterion: "Oi".to_string(),
        verify: false,
        participants: vec![agent.clone()],
        alternate: false,
        variables: Vec::new(),
        stop: None,
    };
    let prompt = StepPrompts::build(&cache, &plain, &agent, "Oi", ToolPromptMode::VokiaCli, None);
    assert!(!prompt.opening.contains("variaveis:"));
    assert!(!prompt.opening.contains("set_variable"));
    assert!(prompt.opening.contains("pedido: Oi"));
    assert_eq!(prompt.opening.matches("Oi").count(), 1);

    let with_vars = crate::task::TaskDefinition {
        kind: crate::task::TaskKind::Demanda,
        nova_tarefa: true,
        objetivo: "Contar até 10".to_string(),
        criterion: "Contar até 10".to_string(),
        verify: false,
        participants: vec![agent.clone()],
        alternate: true,
        variables: vec![crate::task::Variable::system(
            "contagem".to_string(),
            crate::task::VarValue::Number(0),
            crate::task::Rule::Increment,
        )],
        stop: None,
    };
    let prompt_with_vars = StepPrompts::build(
        &cache,
        &with_vars,
        &agent,
        "Comece a contagem",
        ToolPromptMode::Mcp,
        None,
    );
    assert!(prompt_with_vars.opening.contains("variaveis:"));
    assert!(prompt_with_vars.opening.contains("set_variable"));
    assert!(prompt_with_vars.opening.contains("objetivo: Contar até 10"));
    assert!(prompt_with_vars.opening.contains("instrucao: Comece a contagem"));

    let delegated = StepPrompts::build(
        &cache,
        &plain,
        &agent,
        "Axiom pediu: explique o projeto",
        ToolPromptMode::None,
        None,
    );
    assert!(delegated.opening.contains("objetivo: Oi"));
    assert!(delegated.opening.contains("instrucao: Axiom pediu: explique o projeto"));
}

#[test]
fn variavel_compacta_usa_escopo_curto_flag_ro_e_dois_pontos() {
    let team = |rule| {
        crate::task::Variable::system("limite".to_string(), crate::task::VarValue::Number(20), rule)
    };
    assert_eq!(describe_variable(&team(crate::task::Rule::ReadOnly)), "- limite (número, comp, ro): 20");
    assert_eq!(describe_variable(&team(crate::task::Rule::Free)), "- limite (número, comp): 20");

    let mut private = crate::task::Variable::system(
        "rascunho".to_string(),
        crate::task::VarValue::List(vec!["a".to_string(), "b".to_string()]),
        crate::task::Rule::Free,
    );
    private.scope = Scope::Private;
    assert_eq!(describe_variable(&private), "- rascunho (lista, priv): a, b");
}

#[test]
fn step_prompt_com_variaveis_nao_usa_rotulos_antigos_e_traz_objetivo_uma_vez() {
    let agent = Agent::ember();
    let cache = crate::task::Cache::default();
    let definition = crate::task::TaskDefinition {
        kind: crate::task::TaskKind::Demanda,
        nova_tarefa: true,
        objetivo: "Contar".to_string(),
        criterion: "Contar".to_string(),
        verify: false,
        participants: vec![agent.clone()],
        alternate: false,
        variables: vec![crate::task::Variable::system(
            "contagem".to_string(),
            crate::task::VarValue::Number(0),
            crate::task::Rule::ReadOnly,
        )],
        stop: None,
    };
    let opening = StepPrompts::build(&cache, &definition, &agent, "Comece", ToolPromptMode::None, None).opening;
    for legacy in ["Objetivo:", "Variáveis visíveis:", "Instrução:", "Pedido:", "somente leitura"] {
        assert!(!opening.contains(legacy), "{legacy} não deveria aparecer: {opening}");
    }
    assert_eq!(opening.matches("objetivo: Contar").count(), 1);
}

#[test]
fn role_prompt_inclui_regras_de_ferramenta_conforme_tool_mode() {
    with_test_context(|_, profile| {
        // Modo MCP
        let role_mcp = role(profile, None, "Roster.", None, "Contexto.", ToolPromptMode::Mcp, ToolScope::Coordinator, None);
        assert!(role_mcp.contains("app_commands_list"));
        assert!(role_mcp.contains("app_command_call"));

        // Modo VokiaCli
        let role_cli = role(profile, None, "Roster.", None, "Contexto.", ToolPromptMode::VokiaCli, ToolScope::Coordinator, None);
        assert!(role_cli.contains("vokia-cli commands"));
        assert!(role_cli.contains("vokia-cli cmd"));

        // Modo None
        let role_none = role(profile, None, "Roster.", None, "Contexto.", ToolPromptMode::None, ToolScope::Coordinator, None);
        assert!(!role_none.contains("app_commands_list"));
        assert!(!role_none.contains("vokia-cli commands"));
    });
}

#[test]
fn maestro_e_role_orientam_triagem_com_decisao_e_plano_antes_da_implementacao() {
    with_test_context(|team, profile| {
        let choices = ProjectChoices {
            pinned: Some("projeto-a".to_string()),
            labels: vec!["projeto-a".to_string()],
        };
        let maestro_auto = maestro_role(
            "Base Ember.",
            &[],
            &choices,
                        None,
            team,
            &[],
            ToolPromptMode::Mcp,
            true,
        );
        assert!(maestro_auto.contains("triagem"));
        assert!(maestro_auto.contains("request_decision"));
        assert!(maestro_auto.contains("save_item_plan"));
        assert!(maestro_auto.contains(MAESTRO_INSTRUCTIONS_AUTO_TRIAGE));

        let maestro_manual = maestro_role(
            "Base Ember.",
            &[],
            &choices,
                        None,
            team,
            &[],
            ToolPromptMode::Mcp,
            false,
        );
        assert!(maestro_manual.contains("create_card"));
        assert!(maestro_manual.contains("sem delegar a triagem automaticamente ao líder"));
        assert!(maestro_manual.contains(MAESTRO_INSTRUCTIONS_MANUAL_TRIAGE));

        assert!(MAESTRO_TEAM_RULE_AUTO_TRIAGE.contains("triagem"));
        assert!(MAESTRO_TEAM_RULE_AUTO_TRIAGE.contains("request_decision"));
        assert!(MAESTRO_TEAM_RULE_AUTO_TRIAGE.contains("save_item_plan"));
        assert!(MAESTRO_TEAM_RULE_MANUAL_TRIAGE.contains("sem delegar a triagem automaticamente"));
        assert!(MAESTRO_TEAM_RULE_MANUAL_TRIAGE.contains("a menos que o usuário peça explicitamente para triar"));

        let role_mcp = role(profile, None, "Roster.", None, "Contexto.", ToolPromptMode::Mcp, ToolScope::Coordinator, None);
        assert!(role_mcp.contains("triagem"));
        assert!(role_mcp.contains("request_decision"));
        assert!(role_mcp.contains("save_item_plan"));
        assert!(role_mcp.contains(crate::plan_proposal::PLAN_PROTOCOL_HINT));

        let role_cli = role(profile, None, "Roster.", None, "Contexto.", ToolPromptMode::VokiaCli, ToolScope::Coordinator, None);
        assert!(role_cli.contains("triagem"));
        assert!(role_cli.contains("request-decision"));
        assert!(role_cli.contains("save_item_plan"));
        assert!(role_cli.contains(crate::plan_proposal::PLAN_PROTOCOL_HINT));
    });
}

#[test]
fn worker_nao_ve_orientacao_de_ferramentas_que_nao_recebe() {
    with_test_context(|_, profile| {
        let worker = role(profile, None, "Roster.", None, "Contexto.", ToolPromptMode::Mcp, ToolScope::Worker, None);
        assert!(!worker.contains("app_commands_list"), "{worker}");
        assert!(!worker.contains("app_command_call"), "{worker}");
        assert!(worker.contains("delegate"));
        assert!(worker.contains(TOOLS_RULE));

        let coordinator = role(profile, None, "Roster.", None, "Contexto.", ToolPromptMode::Mcp, ToolScope::Coordinator, None);
        assert!(coordinator.contains(ADMIN_TOOLS_RULE));
    });
}

#[test]
fn prompt_da_gitta_traz_o_papel_registrado_entre_identidade_e_camada() {
    use crate::roles::{RoleDraft, RoleRegistry};

    let dir = std::env::temp_dir().join(format!(
        "vokia-prompts-gitta-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("diretório temporário");
    let agents = AgentRegistry::load(&dir);
    let roles = RoleRegistry::load(&dir, &agents);
    roles
        .create(&RoleDraft {
            name: "Git & GitHub Specialist".to_string(),
            description: "Cuida do histórico e do fluxo de PRs.".to_string(),
            instructions: "Condição de fim: a branch foi integrada. Fim de task: remova worktree e branch."
                .to_string(),
        })
        .expect("papel criado");

    let mut gitta = agents.team().expect("team padrão").profiles()[0].clone();
    gitta.name = "Gitta".to_string();
    gitta.role = "Git & GitHub Specialist".to_string();
    gitta.extra_instructions = "Extra da Gitta.".to_string();
    let layer = LayerContext {
        layer_name: "Entrega".to_string(),
        layer_instructions: "Regra da camada.".to_string(),
        agent_instructions: "Regra do agente.".to_string(),
        agent_criteria: String::new(),
        map_rules: None,
    };
    let registered = roles.find(&gitta.role).expect("leitura").expect("papel registrado");

    for mode in [ToolPromptMode::Mcp, ToolPromptMode::VokiaCli, ToolPromptMode::None] {
        let prompt = role(&gitta, Some(&registered), "Roster.", Some(&layer), "Contexto.", mode, ToolScope::Coordinator, None);
        let positions: Vec<usize> = [
            "Você é Gitta, Git & GitHub Specialist.",
            "Condição de fim",
            "Fim de task",
            "Regra da camada.",
            "Regra do agente.",
            "Extra da Gitta.",
        ]
        .iter()
        .map(|marker| prompt.find(marker).unwrap_or_else(|| panic!("{marker} ausente: {prompt}")))
        .collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "ordem {positions:?}: {prompt}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn role_with(description: &str, instructions: &str) -> crate::roles::Role {
    crate::roles::Role {
        id: "role-teste".to_string(),
        name: "Papel de teste".to_string(),
        description: description.to_string(),
        instructions: instructions.to_string(),
    }
}

#[test]
fn papel_sem_texto_ou_ausente_nao_muda_o_prompt() {
    with_test_context(|_, profile| {
        for mode in [ToolPromptMode::Mcp, ToolPromptMode::VokiaCli, ToolPromptMode::None] {
            let baseline = role(profile, None, "Roster.", None, "Contexto.", mode, ToolScope::Coordinator, None);
            let empty = role_with("  ", "\n");
            let with_empty = role(profile, Some(&empty), "Roster.", None, "Contexto.", mode, ToolScope::Coordinator, None);
            assert_eq!(baseline, with_empty);
            assert!(!baseline.contains(ROLE_INSTRUCTIONS_LABEL));
        }
    });
}

#[test]
fn card_da_troca_vai_na_abertura_e_na_recuperacao_mas_nao_no_seguimento() {
    let agent = Agent::axiom();
    let cache = crate::task::Cache::default();
    let definition = crate::task::TaskDefinition {
        kind: crate::task::TaskKind::Demanda,
        nova_tarefa: true,
        objetivo: "Implementar".to_string(),
        criterion: String::new(),
        verify: false,
        participants: vec![agent.clone()],
        alternate: false,
        variables: Vec::new(),
        stop: None,
    };
    let card = "Card VOK-1: Login\nCritério desta etapa: plano-VOK-1 c2";

    let with_card = StepPrompts::build(&cache, &definition, &agent, "Siga", ToolPromptMode::Mcp, Some(card));
    assert!(with_card.opening.contains(card), "{}", with_card.opening);
    assert!(with_card.minimal.contains(card), "{}", with_card.minimal);
    assert!(!with_card.follow_up.contains(card), "{}", with_card.follow_up);

    let without = StepPrompts::build(&cache, &definition, &agent, "Siga", ToolPromptMode::Mcp, None);
    assert!(!without.opening.contains("card:"));
}

#[test]
fn regra_de_fonte_do_papel_vale_nos_tres_modos_de_ferramenta() {
    with_test_context(|_, profile| {
        for mode in [ToolPromptMode::Mcp, ToolPromptMode::VokiaCli, ToolPromptMode::None] {
            let prompt = role(profile, None, "Roster.", None, "Contexto.", mode, ToolScope::Coordinator, None);
            assert!(prompt.contains(ROLE_SOURCE_RULE), "{prompt}");
            assert!(prompt.contains("role_list"));
        }
    });
}

#[test]
fn instrucao_longa_do_papel_entra_inteira_no_prompt() {
    with_test_context(|_, profile| {
        let long = format!("{}FIM-DA-INSTRUCAO", "regra ".repeat(700));
        assert!(long.len() > 4000);
        let registered = role_with("Descrição.", &long);
        let prompt = role(profile, Some(&registered), "Roster.", None, "Contexto.", ToolPromptMode::Mcp, ToolScope::Coordinator, None);
        assert!(prompt.contains(&long));
    });
}

#[test]
fn interpretador_recebe_secao_do_papel_e_a_regra_de_fonte() {
    with_test_context(|team, _| {
        let registered = role_with("Descrição do papel.", "Instrução do papel.");
        let prompt = interpreter_role(team, None, Some(&registered));
        assert!(prompt.contains(&format!("{ROLE_INSTRUCTIONS_LABEL} Papel de teste: Descrição do papel. Instrução do papel.")), "{prompt}");
        assert!(prompt.ends_with(ROLE_SOURCE_RULE), "{prompt}");
    });
}

#[test]
fn interpretador_sem_papel_ou_com_papel_vazio_so_recebe_a_regra() {
    with_test_context(|team, _| {
        let empty = role_with(" ", "\n");
        for registered in [None, Some(&empty)] {
            let prompt = interpreter_role(team, None, registered);
            assert!(!prompt.contains(ROLE_INSTRUCTIONS_LABEL), "{prompt}");
            assert!(prompt.ends_with(ROLE_SOURCE_RULE), "{prompt}");
        }
    });
}

#[test]
fn maestro_recebe_a_secao_do_papel_uma_unica_vez_via_base() {
    with_test_context(|team, profile| {
        let registered = role_with("Descrição do papel.", "Instrução única do papel.");
        let choices = ProjectChoices { pinned: None, labels: vec![] };
        for mode in [ToolPromptMode::Mcp, ToolPromptMode::VokiaCli, ToolPromptMode::None] {
            let base = role(profile, Some(&registered), "Roster.", None, "Contexto.", mode, ToolScope::Coordinator, None);
            let prompt = maestro_role(&base, &[], &choices, None, team, &[], mode, false);
            assert_eq!(prompt.matches("Instrução única do papel.").count(), 1, "{prompt}");
            assert_eq!(prompt.matches(ROLE_INSTRUCTIONS_LABEL).count(), 1, "{prompt}");
            assert!(prompt.contains(ROLE_SOURCE_RULE), "{prompt}");
        }
    });
}

#[test]
fn prompts_diferenciam_citacao_de_nome_e_delegacao_explicita_ou_arroba() {
    with_test_context(|team, profile| {
        for mode in [ToolPromptMode::Mcp, ToolPromptMode::VokiaCli, ToolPromptMode::None] {
            let prompt = role(profile, None, "Roster.", None, "Contexto.", mode, ToolScope::Coordinator, None);
            assert!(prompt.contains("@"), "{prompt}");
            assert!(prompt.contains("citar o nome"), "{prompt}");
        }
        let interp = interpreter_role(team, None, None);
        assert!(interp.contains("@"), "{interp}");
        assert!(interp.contains("citar o nome"), "{interp}");
    });
}
