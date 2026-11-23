use super::*;
use crate::agents::{AgentProfile, AgentRegistry, Team};

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
fn tools_rule_contem_orientacao_mcp_para_app_commands() {
    assert!(TOOLS_RULE.contains("app_commands_list"));
    assert!(TOOLS_RULE.contains("app_command_call"));
    assert!(TOOLS_RULE.contains("opcionalmente filtrando domain"));
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
    let prompt = StepPrompts::build(&cache, &plain, &agent, "Oi", ToolPromptMode::VokiaCli);
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
    let opening = StepPrompts::build(&cache, &definition, &agent, "Comece", ToolPromptMode::None).opening;
    for legacy in ["Objetivo:", "Variáveis visíveis:", "Instrução:", "Pedido:", "somente leitura"] {
        assert!(!opening.contains(legacy), "{legacy} não deveria aparecer: {opening}");
    }
    assert_eq!(opening.matches("objetivo: Contar").count(), 1);
}

#[test]
fn role_prompt_inclui_regras_de_ferramenta_conforme_tool_mode() {
    with_test_context(|_, profile| {
        // Modo MCP
        let role_mcp = role(profile, "Roster.", None, "Contexto.", ToolPromptMode::Mcp);
        assert!(role_mcp.contains("app_commands_list"));
        assert!(role_mcp.contains("app_command_call"));

        // Modo VokiaCli
        let role_cli = role(profile, "Roster.", None, "Contexto.", ToolPromptMode::VokiaCli);
        assert!(role_cli.contains("vokia-cli commands"));
        assert!(role_cli.contains("vokia-cli cmd"));

        // Modo None
        let role_none = role(profile, "Roster.", None, "Contexto.", ToolPromptMode::None);
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

        let role_mcp = role(profile, "Roster.", None, "Contexto.", ToolPromptMode::Mcp);
        assert!(role_mcp.contains("triagem"));
        assert!(role_mcp.contains("request_decision"));
        assert!(role_mcp.contains("save_item_plan"));
        assert!(role_mcp.contains(crate::plan_proposal::PLAN_PROTOCOL_HINT));

        let role_cli = role(profile, "Roster.", None, "Contexto.", ToolPromptMode::VokiaCli);
        assert!(role_cli.contains("triagem"));
        assert!(role_cli.contains("request-decision"));
        assert!(role_cli.contains("save_item_plan"));
        assert!(role_cli.contains(crate::plan_proposal::PLAN_PROTOCOL_HINT));
    });
}
