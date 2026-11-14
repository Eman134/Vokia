use crate::agents::{Agent, AgentProfile};
use crate::cache::{
    Cache, Rule, Scope, Variable, CONVERSATION_ITEMS, CONVERSATION_VAR, RECOVERY_CONVERSATION_ITEMS,
};
use crate::task::TaskDefinition;

use super::project_context::PROJECT_RULE;
use super::NO_CONVERSATION;

const PROMPT_RULES: &str = "Para registrar progresso, escreva uma linha `$nome = valor` por variável alterada. Use `@time` ao criar uma variável que o outro agente deve ler e `$nome += item` para listas. Execute a instrução agora: não se apresente nem comente o processo.";
const DELEGATION_RULE: &str = "Delegue só quando o pedido exigir algo que você não sabe fazer ou quando o usuário pediu outro agente pelo nome; nesse caso termine a resposta com uma linha exatamente assim: @<Nome>: <pedido>. Nunca delegue para confirmar o que você já respondeu, nem uma resposta que você já deu, e nunca escreva uma delegação vazia como \"@<Nome>: nenhum pedido adicional\". Quando a tarefa estiver concluída, responda sem essa linha.";
const DECISION_RULE: &str = "Quando precisar de uma decisão do usuário que não seja executar um comando (aprovar algo, escolher entre opções, responder uma pergunta ou revisar um lote), termine a resposta com uma linha exatamente assim: #decisao <tipo>|<título>|<detalhe>|<opção1>;<opção2>. O tipo é aprovacao, escolha, pergunta ou revisao; detalhe e opções podem ficar vazios. Não escreva nada depois dessa linha. Comandos o app já pergunta sozinho.";
pub(super) const REPLY_PREFIX: &str = " respondeu: ";
pub(super) const ALTERNATE_INSTRUCTION: &str =
    "É a sua vez. Atualize as variáveis conforme o objetivo.";
pub(super) const REJECTION_PREFIX: &str = "Sua resposta anterior foi rejeitada: ";
pub(super) const NO_NAMES: &str = "nenhuma";
const NO_VARIABLES: &str = "- nenhuma";
pub(super) const INTERPRETER_ROLE: &str = "Você extrai a definição de uma tarefa e não conversa. Responda somente com um objeto JSON, sem texto fora dele, com os campos: nova_tarefa (true se o pedido inicia um assunto diferente do anterior, false se continua o anterior); objetivo (frase curta); participantes (só quem precisa agir: por padrão o destinatário, ou o agente que o pedido cita; inclua os dois apenas quando o pedido mandar os dois agirem); alternar (true somente se o pedido pedir revezamento explícito entre os dois, como alternadamente, um de cada vez ou revezando; sem esse pedido é false, e então variaveis é [] e parada é null); variaveis (lista de {\"nome\", \"tipo\": \"numero\" | \"texto\" | \"lista\", \"valor\", \"regra\": \"livre\" | \"mais_um\" | \"so_leitura\"}); parada ({\"variavel\", \"alcanca\": nome de uma variável numérica ou inteiro}, ou null). Exemplos: \"contem até 20 alternadamente\" → {\"nova_tarefa\":true,\"objetivo\":\"contar até 20 alternadamente\",\"participantes\":[\"Iara\",\"Davi\"],\"alternar\":true,\"variaveis\":[{\"nome\":\"contagem\",\"tipo\":\"numero\",\"valor\":0,\"regra\":\"mais_um\"},{\"nome\":\"limite\",\"tipo\":\"numero\",\"valor\":20,\"regra\":\"so_leitura\"}],\"parada\":{\"variavel\":\"contagem\",\"alcanca\":\"limite\"}}. \"fale um animal alternadamente, você e o davi\" → {\"nova_tarefa\":true,\"objetivo\":\"alternar animais\",\"participantes\":[\"Davi\",\"Iara\"],\"alternar\":true,\"variaveis\":[{\"nome\":\"animais\",\"tipo\":\"lista\",\"valor\":[],\"regra\":\"livre\"}],\"parada\":null}. \"quanto é 1+1?\" → {\"nova_tarefa\":true,\"objetivo\":\"responder 1+1\",\"participantes\":[\"Iara\"],\"alternar\":false,\"variaveis\":[],\"parada\":null}. \"conte até 20\" para o Davi, sem pedir revezamento → {\"nova_tarefa\":true,\"objetivo\":\"contar até 20\",\"participantes\":[\"Davi\"],\"alternar\":false,\"variaveis\":[],\"parada\":null}.";

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
    ) -> Self {
        let prompt = |conversation_items, with_request| {
            step_prompt(
                cache,
                definition,
                agent,
                instruction,
                conversation_items,
                with_request,
            )
        };
        Self {
            opening: prompt(CONVERSATION_ITEMS, true),
            follow_up: prompt(NO_CONVERSATION, false),
            minimal: prompt(RECOVERY_CONVERSATION_ITEMS, true),
        }
    }
}

// O estado compartilhado só entra quando há revezamento; fora dele o agente recebe o pedido puro.
fn step_prompt(
    cache: &Cache,
    definition: &TaskDefinition,
    agent: &Agent,
    instruction: &str,
    conversation_items: usize,
    with_request: bool,
) -> String {
    let mut sections = vec![format!("Objetivo: {}", definition.objetivo)];
    if let Some(request) = cache.long_request().filter(|_| with_request) {
        sections.push(format!(
            "Pedido original:
{request}"
        ));
    }
    if definition.alternate {
        sections.push(format!(
            "Variáveis visíveis:\n{}",
            variables_listing(cache, agent)
        ));
    }
    let conversation = cache.recent_conversation(conversation_items);
    if !conversation.is_empty() {
        let entries: Vec<String> = conversation
            .into_iter()
            .map(|entry| format!("- {entry}"))
            .collect();
        sections.push(format!("Conversa recente:\n{}", entries.join("\n")));
    }
    sections.push(format!("Instrução: {instruction}"));
    if definition.alternate {
        sections.push(PROMPT_RULES.to_string());
    }
    sections.join("\n")
}

fn variables_listing(cache: &Cache, agent: &Agent) -> String {
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

pub(super) fn role(profile: &AgentProfile, roster: &str, project_context: &str) -> String {
    format!(
        "Você é {}, {}. {roster} Responda o pedido diretamente, em português, em no máximo 3 frases, salvo quando a instrução pedir só um valor. {DELEGATION_RULE} {DECISION_RULE} {PROJECT_RULE} {project_context}",
        profile.name, profile.role
    )
}
