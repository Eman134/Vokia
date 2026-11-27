use crate::agents::{join_items, Agent, AgentProfile, Team};
use crate::db::OrchestrationSnapshot;
use crate::task::{
    Cache, Rule, Scope, TaskDefinition, Variable, CONVERSATION_ITEMS, CONVERSATION_VAR,
    RECOVERY_CONVERSATION_ITEMS,
};

use crate::orchestration::snapshot::{leader_of, team_outlines, LayerContext, TeamOutline};
use crate::mcp::ToolScope;
use crate::plan_proposal::PLAN_PROTOCOL_HINT;
use crate::roles::Role;
use serde_json::Value;

const MAESTRO_INSTRUCTIONS_AUTO_TRIAGE: &str = "<papel>Orquestrador do Vokia. Encaminhe cada pedido ao líder do time ou agente certo; não execute trabalho técnico.</papel>\n<fluxo>Card novo nasce em triagem: delegue ao líder para consultar o grafo (memory_graph_query), ler o código, registrar contexto/prioridade no grafo (memory_graph_save), usar request_decision se houver ambiguidade real e gravar o plano via save_item_plan antes da implementação. Se o pedido tiver múltiplas frentes independentes, crie os cards e chame delegate em paralelo no mesmo turno para o time trabalhar junto. Delegou, encerre o turno. Respeite camadas e limites.</fluxo>";
const MAESTRO_INSTRUCTIONS_MANUAL_TRIAGE: &str = "<papel>Orquestrador do Vokia. Encaminhe cada pedido ao líder do time ou agente certo; não execute trabalho técnico.</papel>\n<fluxo>Card novo nasce em triagem: crie via create_card sem delegar a triagem automaticamente ao líder, salvo se o usuário pedir explicitamente para triar (nesse caso, delegue ao líder para consultar memory_graph_query, ler o código, registrar descobertas/prioridade via memory_graph_save, usar request_decision se houver ambiguidade real e gravar o plano via save_item_plan antes da implementação). Se o pedido tiver múltiplas frentes independentes, crie os cards e delegue em paralelo no mesmo turno quando aplicável. Delegou, encerre o turno. Respeite camadas e limites.</fluxo>";
const MAESTRO_PROJECT_RULE: &str = "Identifique o projeto do pedido pelo contexto da conversa, que é o projeto em que o pedido foi feito. Sem contexto, use o projeto fixado pelo usuário. Se ainda houver dúvida, não chute: chame a ferramenta request_decision com kind escolha, title com a pergunta em português (por exemplo: Em qual projeto devo seguir com este pedido?) e options com os rótulos dos projetos listados abaixo, exatamente como aparecem (nome e #id).";
const MAESTRO_PINNED_LABEL: &str = "Projeto fixado pelo usuário:";
const MAESTRO_PROJECTS_LABEL: &str = "Projetos disponíveis:";
const MAESTRO_COMMANDS_LABEL: &str = "Comandos disponíveis:";
const MAESTRO_APP_COMMANDS_MCP: &str = " Você administra o Vokia: para gerenciar papéis, agentes, orquestração, provedores, canais, projetos ou configurações, descubra os comandos com app_commands_list (opcionalmente com domain; cada um traz when_to_use e o schema) e execute com app_command_call (name e arguments).";
const MAESTRO_APP_COMMANDS_VOKIA_CLI: &str = " Use `vokia-cli` no terminal. Administre o Vokia: liste comandos com `vokia-cli commands [--domain <dom>]` (traz when_to_use) e execute com `vokia-cli cmd <comando> --json <obj>`.";
const NO_PROJECT_TEXT: &str = "nenhum";
const CRITERIA_LABEL: &str = "Para concluir sua etapa:";
const MAESTRO_TEAMS_LABEL: &str = "Times do Vokia:";
const MAESTRO_TEAM_RULE_AUTO_TRIAGE: &str = "Escolha um time, crie o card (nasce em triagem) e delegue ao líder disponível para triar (ler código, usar request_decision se necessário e save_item_plan) antes da implementação. Em dúvida real entre times, chame request_decision (kind escolha, title em português, options com os nomes dos times).";
const MAESTRO_TEAM_RULE_MANUAL_TRIAGE: &str = "Escolha um time e crie o card (nasce em triagem) sem delegar a triagem automaticamente ao líder, a menos que o usuário peça explicitamente para triar (nesse caso, delegue ao líder disponível para ler o código, usar request_decision se necessário e save_item_plan antes da implementação). Em dúvida real entre times, chame request_decision (kind escolha, title em português, options com os nomes dos times).";
const NO_AVAILABLE_LEADER: &str = "sem líder disponível";
const NO_TEAM_MEMBERS: &str = "nenhum agente posicionado";

use super::project_context::{ProjectChoices, PROJECT_RULE};
use super::turns::NO_CONVERSATION;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolPromptMode {
    None,
    Mcp,
    VokiaCli,
}

const PROMPT_RULES: &str = "Para registrar progresso, escreva uma linha `$nome = valor` por variável alterada. Variáveis da tarefa são compartilhadas por padrão e todos os agentes podem ler e alterar. Use `$nome += item` para listas. Execute a instrução agora: não se apresente nem comente o processo.";
const DELEGATION_RULE: &str = "Delegue só quando o pedido exigir algo que você não sabe fazer, quando o usuário mencionar outro agente com @<Nome> ou pedir explicitamente para delegar a ele (apenas citar o nome de um agente no texto não significa delegação); nesse caso termine a resposta com uma linha exatamente assim: @<Nome>: <pedido>. Nunca delegue para confirmar o que você já respondeu, nem uma resposta que você já deu, e nunca escreva uma delegação vazia como \"@<Nome>: nenhum pedido adicional\". Quando a tarefa estiver concluída, responda sem essa linha.";
const CARD_RULE: &str = "Para registrar ou mover trabalho no backlog do projeto, escreva uma linha por operação: #card criar <título>|<descrição> ou #card mover <id>|<coluna>. As colunas são Backlog, In progress e In review; nunca use Ready nem Done: quem conclui a task é o usuário ou a validação final. Mova só cards seus ou o card do pedido atual. Não repita um card que já existe e não misture linhas #card com #decisao. O app confirma cada operação na conversa.";
const DECISION_RULE: &str = "Quando precisar de uma decisão do usuário que não seja executar um comando (aprovar algo, escolher entre opções, responder uma pergunta ou revisar um lote), termine a resposta com uma linha exatamente assim: #decisao <tipo>|<título>|<detalhe>|<opção1>;<opção2>. O tipo é aprovacao, escolha, pergunta ou revisao; detalhe e opções podem ficar vazios. Não escreva nada depois dessa linha. Comandos o app já pergunta sozinho.";
const PROMPT_RULES_TOOLS: &str = "Aja diretamente sem preâmbulos; execute chamadas de ferramentas independentes em paralelo. Registre progresso com set_variable (name, op, value; op append para listas; share false só para rascunho pessoal) e leia com get_variables. Para delegar a outro agente, chame delegate (agent, request).";
const PROMPT_RULES_NO_VARS: &str = "Aja diretamente no pedido sem preâmbulos. Execute chamadas independentes em paralelo; se uma ferramenta falhar duas vezes pelo mesmo motivo, reporte o bloqueio em vez de repetir.";
const PROMPT_RULES_NO_VARS_MCP: &str = "Aja diretamente no pedido sem preâmbulos; execute chamadas independentes em paralelo. Para delegar a outro agente do time, chame delegate (agent, request).";
const PROMPT_RULES_NO_VARS_VOKIA_CLI: &str = "Aja diretamente no pedido sem preâmbulos. Para delegar a outro agente do time, execute `vokia-cli delegate --agent <nome> --request \"<pedido>\"`.";
const MAESTRO_VOKIA_CLI_COMMANDS: &str = "- list_backlog / read_card / create_card / move_card / update_card: consultar e gerir cards do backlog
- save_item_plan / artifact_set_criterion: gravar plano (título bem resumido, ID público curto e estimativa em minutos para LLM, ex: '~15min') e marcar critérios de um card
- delegate: delegar pedido ao líder de um time (--agent <nome> --request \"<pedido>\" [--item-id <id>])
- request_decision / propose_request: pedir decisão ou propor card ao usuário
- get_orchestration / list_agents: ler mapa de times e elenco (rode `vokia-cli tools` para detalhes)";
const TOOLS_RULE: &str =
    "Use as ferramentas do Vokia para ler e mexer no backlog, pedir uma decisão ao usuário e ler ou gravar as variáveis da tarefa; nunca invente ids.";
const ADMIN_TOOLS_RULE: &str = "Para gerenciar papéis, agentes, orquestração, provedores, canais ou configurações do Vokia, descubra comandos com app_commands_list (opcionalmente filtrando domain) e execute com app_command_call.";
const DELEGATION_TOOL_RULE: &str = "<delegacao>Chame delegate (agent, request; até 5 em paralelo; notify true só para aviso) apenas quando faltar capacidade, quando o usuário mencionar outro agente com @ ou pedir explicitamente para delegar a ele (apenas citar o nome de um agente no texto não significa delegação). Nunca delegue para confirmar resposta já dada. Em triagem de card: consulte memory_graph_query, leia o código, salve contexto/prioridade via memory_graph_save, use request_decision em caso de dúvida real (a menos que o usuário peça para aplicar recomendações e ser tudo automático) e grave o plano via save_item_plan antes de delegar implementação. Concluído ou delegado, encerre.</delegacao>";
const PROMPT_RULES_VOKIA_CLI: &str = "Aja direto. Variáveis: `vokia-cli set-variable --name <n> --op <set|append> --value <v>` (`--int`, `--share`) e `get-variables`.";
const TOOLS_RULE_VOKIA_CLI: &str = "Use `vokia-cli` (`vokia-cli tools`; `vokia-cli commands [--domain <dom>]` + `vokia-cli cmd <comando> --json <obj>`). Sem IDs inventados.";
const DELEGATION_TOOL_RULE_VOKIA_CLI: &str = "<delegacao>`vokia-cli delegate --agent <a> --request \"<p>\"` (`--item-id <id>`) só se faltar capacidade, houver menção com @ ou pedido explícito de delegação (citar o nome não é delegar). Na triagem: grafo, código, `vokia-cli request-decision` se houver dúvida (salvo se o usuário pedir para aplicar recomendações e ser tudo automático) e `save_item_plan`. Encerre após delegar.</delegacao>";
const STAGE_REPORT_RULE: &str = "Ao concluir cada etapa, marque o critério com artifact_set_criterion (artifact_id, criterion_id, done true) e atualize set_variable.";
const STAGE_REPORT_RULE_VOKIA_CLI: &str = "Ao concluir etapa, marque com `vokia-cli set-criterion --artifact-id <id> --criterion-id <c> --done true` e `vokia-cli set-variable`.";
const WEB_SEARCH_HINT_MCP: &str = " Para pesquisar na web, chame `web_search` (query).";
const WEB_SEARCH_HINT_VOKIA_CLI: &str = " Para pesquisar na web, execute `vokia-cli cmd web_search --json '{\"query\":\"...\"}'`.";
const INSTRUCTION_FILES_RULE: &str = "O CLAUDE.md/AGENTS.md da raiz no contexto já vale; não o releia. Ao editar subpasta, leia o dela até a raiz (mais próximo vence). Em conflito, responda BLOQUEADO.";
pub(super) const REJECTION_PREFIX: &str = "Sua resposta anterior foi rejeitada: ";
pub(super) const ALTERNATE_INSTRUCTION: &str =
    "É a sua vez. Atualize as variáveis conforme o objetivo.";
pub(super) const NO_NAMES: &str = "nenhuma";
pub(super) const CHANNEL_NOTE_PREFIX: &str = "Mensagem enviada no canal #";
const NO_VARIABLES: &str = "- nenhuma";
const GLOBAL_INSTRUCTIONS_LABEL: &str = "Instruções globais do usuário";
const ROLE_INSTRUCTIONS_LABEL: &str = "Instruções do papel";
const ROLE_SOURCE_RULE: &str = "Siga só as instruções deste prompt (não busque papéis no repo nem invente caminhos). Se faltar regra, informe e consulte role_list.";
const LAYER_INSTRUCTIONS_LABEL: &str = "Instruções da camada";
const AGENT_INSTRUCTIONS_LABEL: &str = "Instruções do agente";
const EXTRA_INSTRUCTIONS_LABEL: &str = "Instruções extras";
const INTERPRETER_INTRO: &str = "Você extrai a definição de uma tarefa e não conversa. <líder> é o orquestrador máximo do Vokia: os pedidos chegam a ele e ele os encaminha ao time certo. Responda somente com um objeto JSON, sem texto fora dele, com os campos: nova_tarefa (true se o pedido inicia um assunto diferente do anterior, false se continua o anterior); objetivo (frase curta); participantes (só quem precisa agir: por padrão o líder, <líder>";
const INTERPRETER_FIELDS: &str = r#", ou o agente acionado com @ ou que o pedido manda agir explicitamente (apenas citar o nome de um agente no texto não o torna participante); inclua os dois apenas quando o pedido mandar os dois agirem); alternar (true somente se o pedido pedir revezamento explícito entre os dois, como alternadamente, um de cada vez ou revezando; sem esse pedido é false); variaveis (lista de {"nome", "tipo": "numero" | "texto" | "lista", "valor", "regra": "livre" | "mais_um" | "so_leitura"}; use variáveis quando a tarefa tiver um estado a acompanhar, com ou sem revezamento; [] caso contrário); parada ({"variavel", "alcanca": nome de uma variável numérica, inteiro, ou "existe" para parar quando a variável tiver conteúdo}, ou null); criterio (frase curta sobre o que a conversa mostra quando o objetivo estiver cumprido; se o pedido disser "pronto quando…", use essa condição); verificar (true só quando o pedido tem meta verificável de vários passos, como "faça X até Y"; senão false)."#;
const EXAMPLES_LABEL: &str = "Exemplos:";
const TEAMS_LABEL: &str = "Times do projeto:";
const UNASSIGNED_LABEL: &str = "Sem time:";
const ROSTER_LABEL: &str = "Elenco:";
const NO_LEADER: &str = "nenhum";
const NO_MEMBERS: &str = "nenhum";
const LEADER_TOKEN: &str = "<líder>";
const OTHER_TOKEN: &str = "<outro>";
const EXAMPLE_ALTERNATE_COUNT: &str = r#""contem até 20 alternadamente" → {"nova_tarefa":true,"objetivo":"contar até 20 alternadamente","participantes":["<líder>","<outro>"],"alternar":true,"variaveis":[{"nome":"contagem","tipo":"numero","valor":0,"regra":"mais_um"},{"nome":"limite","tipo":"numero","valor":20,"regra":"so_leitura"}],"parada":{"variavel":"contagem","alcanca":"limite"}}"#;
const EXAMPLE_ALTERNATE_ANIMAL: &str = r#""fale um animal alternadamente, você e o <outro>" → {"nova_tarefa":true,"objetivo":"alternar animais","participantes":["<outro>","<líder>"],"alternar":true,"variaveis":[{"nome":"animais","tipo":"lista","valor":[],"regra":"livre"}],"parada":null}"#;
const EXAMPLE_SINGLE: &str = r#""quanto é 1+1?" → {"nova_tarefa":true,"objetivo":"responder 1+1","participantes":["<líder>"],"alternar":false,"variaveis":[],"parada":null}"#;
const EXAMPLE_TARGET_COUNT: &str = r#""conte até 20" para o <outro>, sem pedir revezamento → {"nova_tarefa":true,"objetivo":"contar até 20","participantes":["<outro>"],"alternar":false,"variaveis":[],"parada":null}"#;
const EXAMPLE_TARGET_REPORT: &str = r#""monte o relatório e me avise quando ele existir" para o <outro> → {"nova_tarefa":true,"objetivo":"montar o relatório","participantes":["<outro>"],"alternar":false,"variaveis":[{"nome":"relatorio","tipo":"texto","valor":"","regra":"livre"}],"parada":{"variavel":"relatorio","alcanca":"existe"},"criterio":"o relatório aparece na conversa","verificar":true}"#;

// Exemplos com dois agentes reais (líder e o primeiro outro do elenco); sem outro agente, a alternância não aparece.
pub(super) fn interpreter_role(
    team: &Team,
    orchestration: Option<&OrchestrationSnapshot>,
    registered_role: Option<&Role>,
) -> String {
    let leader = leader_of(orchestration);
    let leader_name = team.name_of(&leader);
    let other = team.profiles().iter().find(|profile| profile.id != leader);
    let other_name = other.map_or(leader_name, |profile| profile.name.as_str());
    let mut examples = Vec::new();
    if other.is_some() {
        examples.extend([EXAMPLE_ALTERNATE_COUNT, EXAMPLE_ALTERNATE_ANIMAL]);
    }
    examples.extend([EXAMPLE_SINGLE, EXAMPLE_TARGET_COUNT, EXAMPLE_TARGET_REPORT]);
    let examples: Vec<String> = examples
        .into_iter()
        .map(|example| {
            example
                .replace(LEADER_TOKEN, leader_name)
                .replace(OTHER_TOKEN, other_name)
        })
        .collect();
    let intro = INTERPRETER_INTRO.replace(LEADER_TOKEN, leader_name);
    let role_section = role_instructions(registered_role);
    format!(
        "{}{} {} {} {}. {role_section}{ROLE_SOURCE_RULE}",
        intro,
        INTERPRETER_FIELDS,
        team_map_text(team, orchestration),
        EXAMPLES_LABEL,
        examples.join(". ")
    )
}

// Sem times cadastrados, o interpretador vê só o elenco (o líder já está na introdução).
fn team_map_text(team: &Team, orchestration: Option<&OrchestrationSnapshot>) -> String {
    let Some(snapshot) = orchestration else {
        return roster_text(team);
    };
    let outlines = team_outlines(snapshot, team, &[]);
    if outlines.teams.is_empty() {
        return roster_text(team);
    }
    let teams: Vec<String> = outlines.teams.iter().map(team_text).collect();
    let mut text = format!("{TEAMS_LABEL} {}.", teams.join("; "));
    if !outlines.unassigned.is_empty() {
        text.push_str(&format!(
            " {UNASSIGNED_LABEL} {}.",
            join_items(&outlines.unassigned)
        ));
    }
    text
}

fn team_text(team: &TeamOutline) -> String {
    let leader = team.leader.as_deref().unwrap_or(NO_LEADER);
    if let Some(instructions) = &team.instructions {
        return format!(
            "{} (líder: {leader}; instruções: {instructions})",
            team.name
        );
    }
    let members: Vec<String> = team
        .members
        .iter()
        .map(|member| format!("{} ({})", member.name, member.layer))
        .collect();
    let members = if members.is_empty() {
        NO_MEMBERS.to_string()
    } else {
        join_items(&members)
    };
    format!("{} (líder: {leader}; agentes: {members})", team.name)
}

// Sem times cadastrados a seção não existe e o Ember segue só com a regra de projeto.
fn maestro_teams_text(
    orchestration: Option<&OrchestrationSnapshot>,
    team: &Team,
    paused: &[Agent],
    auto_triage: bool,
) -> Option<String> {
    let outlines = team_outlines(orchestration?, team, paused);
    if outlines.teams.is_empty() {
        return None;
    }
    let lines: Vec<String> = outlines.teams.iter().map(maestro_team_line).collect();
    let team_rule = if auto_triage {
        MAESTRO_TEAM_RULE_AUTO_TRIAGE
    } else {
        MAESTRO_TEAM_RULE_MANUAL_TRIAGE
    };
    Some(format!(
        "{MAESTRO_TEAMS_LABEL}
{}
{team_rule}",
        lines.join(
            "
"
        )
    ))
}

fn maestro_team_line(team: &TeamOutline) -> String {
    let leader = team.leader.as_deref().unwrap_or(NO_AVAILABLE_LEADER);
    let description = match &team.instructions {
        Some(instructions) => format!("Instruções: {instructions}"),
        None => {
            let members: Vec<String> = team
                .members
                .iter()
                .map(|member| {
                    format!(
                        "{} ({}; camada {}, {})",
                        member.name, member.role, member.layer, member.layer_kind
                    )
                })
                .collect();
            let members = if members.is_empty() {
                NO_TEAM_MEMBERS.to_string()
            } else {
                join_items(&members)
            };
            format!("Composição: {members}")
        }
    };
    format!(
        "- {} ({}) | líder: {leader} | {description}",
        team.id, team.name
    )
}

fn roster_text(team: &Team) -> String {
    let items: Vec<String> = team
        .profiles()
        .iter()
        .map(|profile| format!("{} ({})", profile.name, profile.role))
        .collect();
    format!("{ROSTER_LABEL} {}.", join_items(&items))
}
pub(super) const GOAL_VERIFIER_ROLE: &str = "Você verifica se um objetivo foi atingido. Você não conversa nem executa nada: julga só pela conversa e pelo critério da mensagem. Responda exatamente neste formato: primeira linha, somente ATINGIDO, INCOMPLETO ou SEM_AVANCO; segunda linha, Motivo: <até 200 caracteres>; só quando a primeira linha for INCOMPLETO, terceira linha, Proximo: <instrução curta, até 200 caracteres>. ATINGIDO só quando o critério está cumprido de forma observável na conversa. INCOMPLETO quando falta algo e a tarefa pode seguir. SEM_AVANCO quando a conversa repete o mesmo sem avançar.";

pub(super) struct StepPrompts {
    pub(super) opening: String,
    pub(super) follow_up: String,
    pub(super) minimal: String,
}

impl StepPrompts {
    pub(super) fn build(
        cache: &Cache,
        definition: &TaskDefinition,
        agent: &Agent,
        instruction: &str,
        tool_mode: ToolPromptMode,
        card: Option<&str>,
    ) -> Self {
        let prompt = |conversation_items, with_request| {
            step_prompt(
                cache,
                definition,
                agent,
                instruction,
                conversation_items,
                with_request,
                tool_mode,
                card,
            )
        };
        Self {
            opening: prompt(CONVERSATION_ITEMS, true),
            follow_up: prompt(NO_CONVERSATION, false),
            minimal: prompt(RECOVERY_CONVERSATION_ITEMS, true),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn step_prompt(
    cache: &Cache,
    definition: &TaskDefinition,
    agent: &Agent,
    instruction: &str,
    conversation_items: usize,
    with_request: bool,
    tool_mode: ToolPromptMode,
    card: Option<&str>,
) -> String {
    let has_variables = !definition.variables.is_empty()
        || cache
            .visible_to(agent)
            .into_iter()
            .any(|variable| variable.name != CONVERSATION_VAR);
    // Pedido simples: o objetivo é o próprio pedido, então não repete nem abre bloco de variáveis vazio.
    let plain_request = !has_variables && definition.objetivo.trim() == instruction.trim();
    let mut sections = Vec::new();
    if !plain_request {
        sections.push(format!("objetivo: {}", definition.objetivo));
    }
    if let Some(request) = cache.long_request().filter(|_| with_request) {
        sections.push(format!("pedido_original:\n{request}"));
    }
    // A sessão que já abriu com o card não o recebe de novo; quem precisar do estado atual chama read_card.
    if let Some(card) = card.filter(|_| with_request) {
        sections.push(format!("card:\n{card}"));
    }
    if has_variables {
        sections.push(format!(
            "variaveis:\n{}",
            variables_listing(cache, agent)
        ));
    }
    let conversation = cache.recent_conversation(conversation_items);
    if !conversation.is_empty() {
        let entries: Vec<String> = conversation
            .into_iter()
            .map(|entry| format!("- {entry}"))
            .collect();
        sections.push(format!("conversa:\n{}", entries.join("\n")));
    }
    let label = if plain_request { "pedido" } else { "instrucao" };
    sections.push(format!("{label}: {instruction}"));
    if !with_request {
        return sections.join("
");
    }
    let rules = match (has_variables, tool_mode) {
        (true, ToolPromptMode::Mcp) => PROMPT_RULES_TOOLS,
        (true, ToolPromptMode::VokiaCli) => PROMPT_RULES_VOKIA_CLI,
        (true, ToolPromptMode::None) => PROMPT_RULES,
        (false, ToolPromptMode::Mcp) => PROMPT_RULES_NO_VARS_MCP,
        (false, ToolPromptMode::VokiaCli) => PROMPT_RULES_NO_VARS_VOKIA_CLI,
        (false, ToolPromptMode::None) => PROMPT_RULES_NO_VARS,
    };
    sections.push(rules.to_string());
    sections.join("\n")
}

pub(crate) fn variables_listing(cache: &Cache, agent: &Agent) -> String {
    let lines: Vec<String> = cache
        .visible_to(agent)
        .into_iter()
        .filter(|variable| variable.name != CONVERSATION_VAR)
        .map(describe_variable)
        .collect();
    if lines.is_empty() {
        NO_VARIABLES.to_string()
    } else {
        lines.join("\n")
    }
}

fn describe_variable(variable: &Variable) -> String {
    let read_only = if variable.rule == Rule::ReadOnly {
        ", ro"
    } else {
        ""
    };
    format!(
        "- {} ({}, {}{read_only}): {}",
        variable.name,
        variable.value.type_label(),
        scope_label(variable.scope),
        variable.value.display()
    )
}

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Private => "priv",
        Scope::Team => "comp",
    }
}

// Ember orquestra o programa: as instruções fixas vêm sempre, e os comandos vêm do registro real de ferramentas.
pub(super) fn maestro_role(
    base: &str,
    tools: &[Value],
    choices: &ProjectChoices,
    orchestration: Option<&OrchestrationSnapshot>,
    team: &Team,
    paused: &[Agent],
    tool_mode: ToolPromptMode,
    auto_triage: bool,
) -> String {
    let pinned = choices.pinned.as_deref().unwrap_or(NO_PROJECT_TEXT);
    let labels = if choices.labels.is_empty() {
        NO_PROJECT_TEXT.to_string()
    } else {
        join_items(&choices.labels)
    };
    let projects = format!("{MAESTRO_PINNED_LABEL} {pinned}. {MAESTRO_PROJECTS_LABEL} {labels}.");
    let teams = maestro_teams_text(orchestration, team, paused, auto_triage)
        .map(|text| format!(" {text}"))
        .unwrap_or_default();
    let cli_hint = match tool_mode {
        ToolPromptMode::Mcp => MAESTRO_APP_COMMANDS_MCP,
        ToolPromptMode::VokiaCli => MAESTRO_APP_COMMANDS_VOKIA_CLI,
        ToolPromptMode::None => "",
    };
    let instructions = if auto_triage {
        MAESTRO_INSTRUCTIONS_AUTO_TRIAGE
    } else {
        MAESTRO_INSTRUCTIONS_MANUAL_TRIAGE
    };
    let head =
        format!("{base} {instructions} {MAESTRO_PROJECT_RULE} {projects}{teams}{cli_hint}");
    // O servidor MCP já entrega o schema das ferramentas; no CLI vai só o catálogo compacto das essenciais.
    match tool_mode {
        ToolPromptMode::Mcp => return head,
        ToolPromptMode::VokiaCli => {
            return format!("{head} {MAESTRO_COMMANDS_LABEL}\n{MAESTRO_VOKIA_CLI_COMMANDS}");
        }
        ToolPromptMode::None => {}
    }
    let commands: Vec<String> = tools.iter().filter_map(command_line).collect();
    if commands.is_empty() {
        return head;
    }
    format!("{head} {MAESTRO_COMMANDS_LABEL}\n{}", commands.join("\n"))
}

fn command_line(tool: &Value) -> Option<String> {
    Some(format!(
        "- {}: {}",
        tool.get("name")?.as_str()?,
        tool.get("description")?.as_str()?
    ))
}

pub(super) fn web_search_prompt_hint(
    tool_mode: ToolPromptMode,
    has_native_web_search: bool,
    web_search_configured: bool,
) -> &'static str {
    if has_native_web_search || !web_search_configured {
        return "";
    }
    match tool_mode {
        ToolPromptMode::Mcp => WEB_SEARCH_HINT_MCP,
        ToolPromptMode::VokiaCli => WEB_SEARCH_HINT_VOKIA_CLI,
        ToolPromptMode::None => "",
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn role_with_options(
    profile: &AgentProfile,
    registered_role: Option<&Role>,
    roster: &str,
    layer: Option<&LayerContext>,
    project_context: &str,
    tool_mode: ToolPromptMode,
    scope: ToolScope,
    global_instructions: Option<&str>,
    has_native_web_search: bool,
    web_search_configured: bool,
) -> String {
    let base = role(
        profile,
        registered_role,
        roster,
        layer,
        project_context,
        tool_mode,
        scope,
        global_instructions,
    );
    let hint = web_search_prompt_hint(tool_mode, has_native_web_search, web_search_configured);
    if hint.is_empty() {
        base
    } else {
        format!("{base}{hint}")
    }
}

pub(super) fn role(
    profile: &AgentProfile,
    registered_role: Option<&Role>,
    roster: &str,
    layer: Option<&LayerContext>,
    project_context: &str,
    tool_mode: ToolPromptMode,
    scope: ToolScope,
    global_instructions: Option<&str>,
) -> String {
    // Sem o servidor MCP/Vokia CLI a sessão volta a ensinar os marcadores de texto.
    let tools_rules = match tool_mode {
        ToolPromptMode::Mcp => {
            let admin = match scope {
                ToolScope::Coordinator => format!(" {ADMIN_TOOLS_RULE}"),
                ToolScope::Worker => String::new(),
            };
            format!("{DELEGATION_TOOL_RULE} {STAGE_REPORT_RULE} {PLAN_PROTOCOL_HINT} {TOOLS_RULE}{admin} {ROLE_SOURCE_RULE}")
        }
        ToolPromptMode::VokiaCli => {
            format!("{DELEGATION_TOOL_RULE_VOKIA_CLI} {STAGE_REPORT_RULE_VOKIA_CLI} {PLAN_PROTOCOL_HINT} {TOOLS_RULE_VOKIA_CLI} {ROLE_SOURCE_RULE}")
        }
        ToolPromptMode::None => format!("{DELEGATION_RULE} {DECISION_RULE} {CARD_RULE} {ROLE_SOURCE_RULE}"),
    };
    let global_section = global_instructions_section(global_instructions);
    let role_section = role_instructions(registered_role);
    let layer_sections = layer_sections(layer);
    let extra = extra_instructions(profile);
    format!(
        "Você é {}, {}. {global_section}{role_section}{roster} Responda o pedido diretamente, em português, em no máximo 3 frases, salvo quando a instrução pedir só um valor. {tools_rules} {layer_sections}{extra}{PROJECT_RULE} {INSTRUCTION_FILES_RULE} {project_context}",
        profile.name, profile.role
    )
}

// Sem arquivo global (ou vazio), o prompt sai igual ao de antes.
fn global_instructions_section(text: Option<&str>) -> String {
    match text.map(str::trim).filter(|text| !text.is_empty()) {
        Some(text) => format!("{GLOBAL_INSTRUCTIONS_LABEL}: {text} "),
        None => String::new(),
    }
}

// Papel sem descrição nem instruções não gera seção.
fn role_instructions(role: Option<&Role>) -> String {
    let Some(role) = role else {
        return String::new();
    };
    let texts: Vec<&str> = [role.description.trim(), role.instructions.trim()]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect();
    if texts.is_empty() {
        return String::new();
    }
    format!("{ROLE_INSTRUCTIONS_LABEL} {}: {} ", role.name, texts.join(" "))
}

// Sem instruções extras, o prompt sai igual ao de antes.
fn extra_instructions(profile: &AgentProfile) -> String {
    if profile.extra_instructions.is_empty() {
        return String::new();
    }
    format!(
        " {EXTRA_INSTRUCTIONS_LABEL}: {}",
        profile.extra_instructions
    )
}

// Cada seção só existe com texto; sem nenhuma, o resultado é vazio e o prompt segue igual ao de antes.
fn layer_sections(layer: Option<&LayerContext>) -> String {
    let Some(layer) = layer else {
        return String::new();
    };
    let mut sections = Vec::new();
    if !layer.layer_instructions.is_empty() {
        sections.push(format!(
            "{LAYER_INSTRUCTIONS_LABEL} {}: {}",
            layer.layer_name, layer.layer_instructions
        ));
    }
    if !layer.agent_instructions.is_empty() {
        sections.push(format!(
            "{AGENT_INSTRUCTIONS_LABEL}: {}",
            layer.agent_instructions
        ));
    }
    if !layer.agent_criteria.is_empty() {
        sections.push(format!("{CRITERIA_LABEL} {}", layer.agent_criteria));
    }
    if let Some(rules) = &layer.map_rules {
        sections.push(rules.clone());
    }
    sections
        .iter()
        .map(|section| format!("{section} "))
        .collect()
}

pub(super) fn channel_note(name: &str, member_names: &[String]) -> String {
    format!(
        "{CHANNEL_NOTE_PREFIX}{name} (membros: {}).",
        member_names.join(", ")
    )
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
