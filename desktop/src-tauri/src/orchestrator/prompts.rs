use crate::agents::{Agent, AgentProfile};
use crate::task::{
    Cache, Rule, Scope, TaskDefinition, Variable, CONVERSATION_ITEMS, CONVERSATION_VAR,
    RECOVERY_CONVERSATION_ITEMS,
};

use super::project_context::PROJECT_RULE;
use super::turns::NO_CONVERSATION;

const PROMPT_RULES: &str = "Para registrar progresso, escreva uma linha `$nome = valor` por variável alterada. Use `@time` ao criar uma variável que o outro agente deve ler e `$nome += item` para listas. Execute a instrução agora: não se apresente nem comente o processo.";
const DELEGATION_RULE: &str = "Delegue só quando o pedido exigir algo que você não sabe fazer ou quando o usuário pediu outro agente pelo nome; nesse caso termine a resposta com uma linha exatamente assim: @<Nome>: <pedido>. Nunca delegue para confirmar o que você já respondeu, nem uma resposta que você já deu, e nunca escreva uma delegação vazia como \"@<Nome>: nenhum pedido adicional\". Quando a tarefa estiver concluída, responda sem essa linha.";
const CARD_RULE: &str = "Para registrar ou mover trabalho no backlog do projeto, escreva uma linha por operação: #card criar <título>|<descrição> ou #card mover <id>|<coluna>. As colunas são Backlog, In progress, In review e Done; nunca use Ready. Mova só cards seus ou o card do pedido atual; Done não volta. Não repita um card que já existe e não misture linhas #card com #decisao. O app confirma cada operação na conversa.";
const DECISION_RULE: &str = "Quando precisar de uma decisão do usuário que não seja executar um comando (aprovar algo, escolher entre opções, responder uma pergunta ou revisar um lote), termine a resposta com uma linha exatamente assim: #decisao <tipo>|<título>|<detalhe>|<opção1>;<opção2>. O tipo é aprovacao, escolha, pergunta ou revisao; detalhe e opções podem ficar vazios. Não escreva nada depois dessa linha. Comandos o app já pergunta sozinho.";
const PROMPT_RULES_TOOLS: &str = "Para registrar progresso, chame a ferramenta set_variable (name, op, value) por variável alterada; use op append para listas, share para uma variável que o outro agente deve ler e get_variables para ler. Execute a instrução agora: não se apresente nem comente o processo. Para pedir algo a outro agente do time, escreva uma linha que começa com @Nome: seguida do pedido.";
const TOOLS_RULE: &str =
    "Use as ferramentas do Vokia para ler e mexer no backlog, pedir uma decisão ao usuário e ler ou gravar as variáveis da tarefa; nunca invente ids.";
const DELEGATION_TOOL_RULE: &str = "Para pedir algo a outro agente do time, escreva uma linha que começa com @Nome: seguida do pedido. Delegue só quando o pedido exigir algo que você não sabe fazer ou quando o usuário pediu outro agente pelo nome; nunca delegue para confirmar o que você já respondeu. Depois de delegar, termine a resposta.";
pub(super) const REJECTION_PREFIX: &str = "Sua resposta anterior foi rejeitada: ";
pub(super) const ALTERNATE_INSTRUCTION: &str =
    "É a sua vez. Atualize as variáveis conforme o objetivo.";
pub(super) const NO_NAMES: &str = "nenhuma";
pub(super) const CHANNEL_NOTE_PREFIX: &str = "Mensagem enviada no canal #";
const NO_VARIABLES: &str = "- nenhuma";
pub(super) const INTERPRETER_ROLE: &str = "Você extrai a definição de uma tarefa e não conversa. Responda somente com um objeto JSON, sem texto fora dele, com os campos: nova_tarefa (true se o pedido inicia um assunto diferente do anterior, false se continua o anterior); objetivo (frase curta); participantes (só quem precisa agir: por padrão o destinatário, ou o agente que o pedido cita; inclua os dois apenas quando o pedido mandar os dois agirem); alternar (true somente se o pedido pedir revezamento explícito entre os dois, como alternadamente, um de cada vez ou revezando; sem esse pedido é false); variaveis (lista de {\"nome\", \"tipo\": \"numero\" | \"texto\" | \"lista\", \"valor\", \"regra\": \"livre\" | \"mais_um\" | \"so_leitura\"}; use variáveis quando a tarefa tiver um estado a acompanhar, com ou sem revezamento; [] caso contrário); parada ({\"variavel\", \"alcanca\": nome de uma variável numérica, inteiro, ou \"existe\" para parar quando a variável tiver conteúdo}, ou null); criterio (frase curta sobre o que a conversa mostra quando o objetivo estiver cumprido; se o pedido disser \"pronto quando…\", use essa condição); verificar (true só quando o pedido tem meta verificável de vários passos, como \"faça X até Y\"; senão false). Exemplos: \"contem até 20 alternadamente\" → {\"nova_tarefa\":true,\"objetivo\":\"contar até 20 alternadamente\",\"participantes\":[\"Iara\",\"Davi\"],\"alternar\":true,\"variaveis\":[{\"nome\":\"contagem\",\"tipo\":\"numero\",\"valor\":0,\"regra\":\"mais_um\"},{\"nome\":\"limite\",\"tipo\":\"numero\",\"valor\":20,\"regra\":\"so_leitura\"}],\"parada\":{\"variavel\":\"contagem\",\"alcanca\":\"limite\"}}. \"fale um animal alternadamente, você e o davi\" → {\"nova_tarefa\":true,\"objetivo\":\"alternar animais\",\"participantes\":[\"Davi\",\"Iara\"],\"alternar\":true,\"variaveis\":[{\"nome\":\"animais\",\"tipo\":\"lista\",\"valor\":[],\"regra\":\"livre\"}],\"parada\":null}. \"quanto é 1+1?\" → {\"nova_tarefa\":true,\"objetivo\":\"responder 1+1\",\"participantes\":[\"Iara\"],\"alternar\":false,\"variaveis\":[],\"parada\":null}. \"conte até 20\" para o Davi, sem pedir revezamento → {\"nova_tarefa\":true,\"objetivo\":\"contar até 20\",\"participantes\":[\"Davi\"],\"alternar\":false,\"variaveis\":[],\"parada\":null}. \"monte o relatório e me avise quando ele existir\" para o Davi → {\"nova_tarefa\":true,\"objetivo\":\"montar o relatório\",\"participantes\":[\"Davi\"],\"alternar\":false,\"variaveis\":[{\"nome\":\"relatorio\",\"tipo\":\"texto\",\"valor\":\"\",\"regra\":\"livre\"}],\"parada\":{\"variavel\":\"relatorio\",\"alcanca\":\"existe\"},\"criterio\":\"o relatório aparece na conversa\",\"verificar\":true}.";
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
        with_tools: bool,
    ) -> Self {
        let prompt = |conversation_items, with_request| {
            step_prompt(
                cache,
                definition,
                agent,
                instruction,
                conversation_items,
                with_request,
                with_tools,
            )
        };
        Self {
            opening: prompt(CONVERSATION_ITEMS, true),
            follow_up: prompt(NO_CONVERSATION, false),
            minimal: prompt(RECOVERY_CONVERSATION_ITEMS, true),
        }
    }
}

fn step_prompt(
    cache: &Cache,
    definition: &TaskDefinition,
    agent: &Agent,
    instruction: &str,
    conversation_items: usize,
    with_request: bool,
    with_tools: bool,
) -> String {
    let mut sections = vec![format!("Objetivo: {}", definition.objetivo)];
    if let Some(request) = cache.long_request().filter(|_| with_request) {
        sections.push(format!(
            "Pedido original:
{request}"
        ));
    }
    sections.push(format!(
        "Variáveis visíveis:\n{}",
        variables_listing(cache, agent)
    ));
    let conversation = cache.recent_conversation(conversation_items);
    if !conversation.is_empty() {
        let entries: Vec<String> = conversation
            .into_iter()
            .map(|entry| format!("- {entry}"))
            .collect();
        sections.push(format!("Conversa recente:\n{}", entries.join("\n")));
    }
    sections.push(format!("Instrução: {instruction}"));
    sections.push(
        if with_tools {
            PROMPT_RULES_TOOLS
        } else {
            PROMPT_RULES
        }
        .to_string(),
    );
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
        ", somente leitura"
    } else {
        ""
    };
    format!(
        "- {} ({}, {}{read_only}) = {}",
        variable.name,
        variable.value.type_label(),
        scope_label(variable.scope),
        variable.value.display()
    )
}

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Private => "privada",
        Scope::Team => "compartilhada",
    }
}

pub(super) fn role(
    profile: &AgentProfile,
    roster: &str,
    project_context: &str,
    with_tools: bool,
) -> String {
    // Sem o servidor MCP a sessão volta a ensinar os marcadores de texto.
    let tools_rules = if with_tools {
        format!("{DELEGATION_TOOL_RULE} {TOOLS_RULE}")
    } else {
        format!("{DELEGATION_RULE} {DECISION_RULE} {CARD_RULE}")
    };
    format!(
        "Você é {}, {}. {roster} Responda o pedido diretamente, em português, em no máximo 3 frases, salvo quando a instrução pedir só um valor. {tools_rules} {PROJECT_RULE} {project_context}",
        profile.name, profile.role
    )
}

pub(super) fn channel_note(name: &str, member_names: &[String]) -> String {
    format!(
        "{CHANNEL_NOTE_PREFIX}{name} (membros: {}).",
        member_names.join(", ")
    )
}
