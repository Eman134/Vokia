use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::db::ARTIFACT_KINDS;
use crate::events::Agent;
use crate::plan_proposal::PLAN_PROTOCOL_HINT;
use crate::projects::ProjectError;

use super::{
    agents, artifacts, attachments, backlog, channels, execution_mode, orchestration, projects, providers,
    roles, settings, team, usage,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exposure {
    AppAndCli,
    UiOnly,
}

pub struct CommandMeta {
    pub domain: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub when_to_use: &'static str,
    pub exposure: Exposure,
    pub schema: fn() -> Value,
}

const UI_ONLY_ERROR: &str = "Comando exclusivo da interface gráfica (não disponível via MCP/CLI).";
pub(crate) const APP_PREFIX: &str = "app:";

const fn exposed(
    domain: &'static str,
    name: &'static str,
    description: &'static str,
    when_to_use: &'static str,
    schema: fn() -> Value,
) -> CommandMeta {
    CommandMeta {
        domain,
        name,
        description,
        when_to_use,
        exposure: Exposure::AppAndCli,
        schema,
    }
}

const fn ui_only(
    domain: &'static str,
    name: &'static str,
    description: &'static str,
) -> CommandMeta {
    CommandMeta {
        domain,
        name,
        description,
        when_to_use: "",
        exposure: Exposure::UiOnly,
        schema: empty_object_schema,
    }
}

fn empty_object_schema() -> Value {
    json!({ "type": "object" })
}

fn obj_schema(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

fn schema_id() -> Value {
    obj_schema(json!({ "id": { "type": "string" } }), &["id"])
}

fn schema_draft() -> Value {
    obj_schema(json!({ "draft": { "type": "object" } }), &["draft"])
}

fn schema_agent() -> Value {
    obj_schema(json!({ "agent": { "type": "string" } }), &["agent"])
}

fn schema_project_id() -> Value {
    obj_schema(
        json!({ "projectId": { "type": "integer" } }),
        &["projectId"],
    )
}

fn schema_link_git_ref() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "itemId": { "type": "string" },
            "kind": { "type": "string", "enum": ["commit", "branch", "pr"] },
            "refId": { "type": "string" },
            "label": { "type": "string" },
            "url": { "type": "string" },
        }),
        &["projectId", "itemId", "kind", "refId"],
    )
}

fn schema_unlink_git_ref() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "itemId": { "type": "string" },
            "linkId": { "type": "string" },
        }),
        &["projectId", "itemId", "linkId"],
    )
}

fn schema_list_git_refs() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "limit": { "type": "integer" },
        }),
        &["projectId"],
    )
}

fn schema_read_attachment_preview() -> Value {
    obj_schema(json!({ "path": { "type": "string" } }), &["path"])
}

fn schema_role_create() -> Value {
    obj_schema(
        json!({ "draft": {
            "type": "object",
            "description": "Objeto RoleDraft (name, description, instructions)",
        } }),
        &["draft"],
    )
}

fn schema_id_edit() -> Value {
    obj_schema(
        json!({ "id": { "type": "string" }, "edit": { "type": "object" } }),
        &["id", "edit"],
    )
}

fn schema_id_draft() -> Value {
    obj_schema(
        json!({ "id": { "type": "string" }, "draft": { "type": "object" } }),
        &["id", "draft"],
    )
}

fn schema_set_agent_instructions() -> Value {
    obj_schema(
        json!({ "id": { "type": "string" }, "text": { "type": "string" } }),
        &["id", "text"],
    )
}

fn schema_set_temporary_override() -> Value {
    obj_schema(
        json!({
            "agent": { "type": "string" },
            "cli": { "type": "string" },
            "modelTier": { "type": "string" },
            "model": { "type": "string" },
            "reason": { "type": "string" },
            "expiresInMs": { "type": "integer" },
        }),
        &["agent", "cli"],
    )
}

fn schema_update_ember_llm() -> Value {
    obj_schema(
        json!({
            "cli": { "type": "string" },
            "modelTier": { "type": "string" },
            "model": { "type": "string" },
            "fallback": { "type": "array" },
        }),
        &[],
    )
}

fn schema_reorder_layers() -> Value {
    obj_schema(
        json!({ "ids": { "type": "array", "items": { "type": "string" } } }),
        &["ids"],
    )
}

fn schema_save_team() -> Value {
    obj_schema(
        json!({ "draft": { "type": "object" }, "leaderAgent": { "type": "string" } }),
        &["draft"],
    )
}

fn schema_unplace_agent() -> Value {
    obj_schema(json!({ "agentId": { "type": "string" } }), &["agentId"])
}

fn schema_save_team_flow() -> Value {
    obj_schema(
        json!({ "teamId": { "type": "string" }, "draft": { "type": "object" } }),
        &["teamId", "draft"],
    )
}

fn schema_set_final_validation() -> Value {
    obj_schema(
        json!({ "teamId": { "type": "string" }, "mode": { "type": "string" } }),
        &["teamId", "mode"],
    )
}

fn schema_set_judge_full_flow() -> Value {
    obj_schema(
        json!({ "teamId": { "type": "string" }, "enabled": { "type": "boolean" } }),
        &["teamId", "enabled"],
    )
}

fn schema_save_orchestration_settings() -> Value {
    obj_schema(json!({ "settings": { "type": "object" } }), &["settings"])
}

fn schema_save_provider() -> Value {
    obj_schema(json!({ "config": { "type": "object" } }), &["config"])
}

fn schema_inspect_cli() -> Value {
    obj_schema(json!({ "command": { "type": "string" } }), &["command"])
}

fn schema_test_provider_config() -> Value {
    obj_schema(
        json!({ "config": { "type": "object" }, "model": { "type": "string" } }),
        &["config"],
    )
}

fn schema_create_channel() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "name": { "type": "string" },
            "members": { "type": "array", "items": { "type": "string" } },
        }),
        &["projectId", "name", "members"],
    )
}

fn schema_update_channel() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "id": { "type": "integer" },
            "name": { "type": "string" },
            "members": { "type": "array", "items": { "type": "string" } },
        }),
        &["projectId", "id", "name", "members"],
    )
}

fn schema_delete_channel() -> Value {
    obj_schema(
        json!({ "projectId": { "type": "integer" }, "id": { "type": "integer" } }),
        &["projectId", "id"],
    )
}

fn schema_set_setting() -> Value {
    obj_schema(
        json!({ "key": { "type": "string" }, "value": { "type": "string" } }),
        &["key", "value"],
    )
}

fn schema_artifact_ref() -> Value {
    obj_schema(
        json!({ "projectId": { "type": "integer" }, "id": { "type": "string" } }),
        &["projectId", "id"],
    )
}

fn artifact_kind_names() -> Vec<&'static str> {
    ARTIFACT_KINDS.iter().map(|kind| kind.kind).collect()
}

fn schema_artifact_create() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "kind": { "type": "string", "enum": artifact_kind_names() },
            "title": { "type": "string" },
            "summary": { "type": "string" },
            "body": { "type": "string", "description": "Markdown; sem body usa o template do tipo" },
            "tags": { "type": "array", "items": { "type": "string" } },
            "author": { "type": "string" },
        }),
        &["projectId", "kind", "title"],
    )
}

fn schema_artifact_update() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "id": { "type": "string" },
            "expectedUpdatedAt": { "type": "integer" },
            "title": { "type": "string" },
            "summary": { "type": "string" },
            "body": { "type": "string" },
            "tags": { "type": "array", "items": { "type": "string" } },
        }),
        &["projectId", "id"],
    )
}

fn schema_artifact_check() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "id": { "type": "string" },
            "index": { "type": "integer", "minimum": 0 },
            "done": { "type": "boolean" },
        }),
        &["projectId", "id", "index", "done"],
    )
}

fn schema_set_artifact_criterion() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "artifactId": { "type": "string" },
            "criterionId": { "type": "string" },
            "done": { "type": "boolean" },
        }),
        &["projectId", "artifactId", "criterionId", "done"],
    )
}

fn schema_save_item_plan() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "itemId": { "type": "string" },
            "plan": {
                "type": "object",
                "properties": {
                    "state": { "type": "string", "description": "vazio | proposto | vinculado" },
                    "proposal": { "type": "string", "description": PLAN_PROTOCOL_HINT },
                    "plan": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "name": { "type": "string" },
                            "meta": { "type": "string" },
                        },
                    },
                    "agent": { "type": "string" },
                },
                "required": ["state"],
            },
        }),
        &["projectId", "itemId", "plan"],
    )
}

fn schema_update_item_plan() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "itemId": { "type": "string" },
            "proposal": { "type": "string", "description": "Texto completo do plano no protocolo #plano-pronto" },
            "expectedUpdatedAt": { "type": "integer" },
        }),
        &["projectId", "itemId", "proposal"],
    )
}

fn schema_import_plan() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "title": { "type": "string" },
            "proposal": { "type": "string", "description": "Plano; o protocolo com #lote é o formato recomendado (gera etapas e fica melhor apresentado), mas qualquer texto é aceito" },
            "author": { "type": "string" },
        }),
        &["projectId", "title", "proposal"],
    )
}

fn schema_erase_all_data() -> Value {
    obj_schema(json!({ "confirm": { "type": "boolean" } }), &["confirm"])
}

fn schema_plan_usage() -> Value {
    obj_schema(
        json!({
            "providerId": {
                "type": "string",
                "description": "Id opcional do provedor (ex.: claude, codex, opencode, gemini) para filtrar o snapshot."
            }
        }),
        &[],
    )
}

const COMMANDS: &[CommandMeta] = &[
    exposed("team", "cli_problem", "Problema detectado ao localizar a CLI do Claude, se houver.", "Para diagnosticar por que nenhum agente consegue rodar: devolve o problema ao localizar a CLI do Claude, ou nada se estiver tudo certo.", empty_object_schema),
    exposed("team", "send_request", "Envia um pedido de texto a um agente de um projeto.", "Para enviar um pedido a um agente em nome do usuário. Exige o projeto e o agente destino; use depois de conferir o projeto com list_projects.", empty_object_schema),
    exposed("team", "answer_permission", "Responde a um pedido de permissão pendente. Parâmetros: id, allow, always, root (opcional; com always grava a regra do comando raiz \"<raiz> *\" e vale para qualquer argumento), reason (opcional, só na negação).", "Para aprovar ou negar um pedido de permissão pendente de uma ferramenta; precisa do id do pedido.", empty_object_schema),
    exposed("team", "answer_decision", "Responde a uma decisão pendente do fluxo.", "Para responder uma decisão pendente pedida ao usuário; precisa do id da decisão e do texto da resposta.", empty_object_schema),
    exposed("team", "list_agent_pauses", "Lista os agentes pausados por projeto.", "Para saber quais agentes estão pausados em cada projeto antes de delegar trabalho.", empty_object_schema),
    exposed("team", "list_interrupted_runs", "Lista as execuções interrompidas.", "Para ver execuções interrompidas que ainda podem ser retomadas ou descartadas.", empty_object_schema),
    exposed("team", "resume_interrupted_run", "Retoma uma execução interrompida de onde parou, sem reenviar o pedido: entrega ao agente original (ou ao Ember, se já houve delegações) o estado parcial com delegações, mensagens e ferramentas já registradas. Parâmetro: id (de list_interrupted_runs). Devolve o RequestOutcome.", "Para continuar uma execução interrompida; liste antes com list_interrupted_runs e use o id dela.", empty_object_schema),
    exposed("team", "discard_interrupted_run", "Descarta uma execução interrompida.", "Para abandonar uma execução interrompida que não deve mais continuar.", empty_object_schema),
    exposed("team", "set_agent_paused", "Pausa ou retoma um agente em um projeto.", "Para pausar ou retomar um agente em um projeto sem excluí-lo.", empty_object_schema),
    exposed("team", "list_queued_requests", "Lista os pedidos persistidos na fila de entrada.", "Para consultar os pedidos aguardando envio na fila.", empty_object_schema),
    exposed("team", "enqueue_queued_request", "Adiciona um pedido na fila persistida. Parâmetros: projectId, to, text, channelId (opcional), itemId (opcional), attachments (opcional).", "Para enfileirar um pedido persistido no banco.", empty_object_schema),
    exposed("team", "update_queued_request", "Atualiza o texto de um pedido na fila. Parâmetros: id, text.", "Para editar um pedido ainda na fila.", empty_object_schema),
    exposed("team", "move_queued_request", "Move um pedido na fila para cima ou para baixo. Parâmetros: id, direction ('up' ou 'down').", "Para reordenar a fila de pedidos.", empty_object_schema),
    exposed("team", "remove_queued_request", "Remove um pedido da fila persistida. Parâmetro: id.", "Para cancelar ou remover um pedido da fila.", empty_object_schema),
    exposed("team", "check_connection", "Verifica se o provedor padrão está acessível.", "Para verificar se o provedor padrão responde antes de culpar o agente por uma falha.", empty_object_schema),
    exposed("team", "agent_console", "Devolve o console recente de um agente em um projeto.", "Para inspecionar as últimas linhas do console de um agente em um projeto ao investigar um erro.", empty_object_schema),
    exposed("projects", "list_projects", "Lista os projetos.", "Para descobrir os projetos e seus ids; use antes de qualquer comando que peça projectId.", empty_object_schema),
    exposed("projects", "select_project", "Seleciona o projeto ativo.", "Para trocar o projeto ativo do usuário; use só quando o usuário pedir.", empty_object_schema),
    ui_only("projects", "pick_folder", "Abre o seletor de pasta do sistema."),
    ui_only("permissions", "set_execution_mode", "Define o modo de execução de um projeto: manual, planejamento, auto ou bypass (só a interface)."),
    exposed("permissions", "get_execution_mode", "Devolve o modo de execução de um projeto: manual, planejamento, auto ou bypass. Parâmetro: projectId.", "Para saber como o projeto trata os pedidos de permissão dos agentes (perguntando, só lendo, aprovando o seguro ou aprovando tudo).", schema_project_id),
    exposed("projects", "inspect_folder", "Inspeciona uma pasta local para criar um projeto.", "Para ver o que há numa pasta local antes de criar um projeto a partir dela.", empty_object_schema),
    exposed("projects", "inspect_github_repo", "Inspeciona um repositório do GitHub para criar um projeto.", "Para validar um repositório do GitHub antes de criar um projeto a partir dele.", empty_object_schema),
    exposed("projects", "create_project", "Cria um projeto.", "Para criar um projeto novo, depois de inspecionar a pasta ou o repositório.", empty_object_schema),
    exposed("projects", "update_project", "Edita um projeto.", "Para alterar dados de um projeto existente (nome, cor, ícone etc.); precisa do id.", empty_object_schema),
    exposed("projects", "delete_project", "Exclui um projeto.", "Para excluir um projeto e seus dados; ação destrutiva, confirme com o usuário antes.", empty_object_schema),
    exposed("projects", "pin_project", "Fixa ou desafixa um projeto.", "Para fixar um projeto como padrão do orquestrador ou desafixar.", empty_object_schema),
    exposed("projects", "clear_conversation", "Apaga a conversa de um projeto.", "Para apagar a conversa de um projeto; ação destrutiva, confirme com o usuário antes.", empty_object_schema),
    exposed("projects", "load_history", "Carrega o histórico de eventos de um projeto (mensagens incluem channelId e itemId quando vinculados).", "Para ler o histórico de eventos de um projeto ao reconstruir o contexto.", empty_object_schema),
    exposed("projects", "set_permission_level", "Define o nível de permissão de um projeto.", "Para mudar o nível de permissão dos agentes em um projeto; recusa se houver fluxo em andamento.", empty_object_schema),
    exposed("projects", "list_permission_rules", "Lista as regras de permissão de um projeto.", "Para ver as regras de permissão já gravadas em um projeto.", empty_object_schema),
    exposed("projects", "delete_permission_rule", "Remove uma regra de permissão de um projeto.", "Para remover uma regra de permissão gravada; precisa da ferramenta e do comando exatos.", empty_object_schema),
    exposed("channels", "list_channels", "Lista os canais de um projeto.", "Para ver os canais de um projeto e seus membros.", schema_project_id),
    exposed("channels", "create_channel", "Cria um canal em um projeto.", "Para criar um canal com pelo menos 2 agentes; o nome usa 2 a 32 letras minúsculas, números ou hífen.", schema_create_channel),
    exposed("channels", "update_channel", "Edita o nome e os agentes de um canal.", "Para renomear um canal ou mudar seus membros; precisa do projectId e do id do canal.", schema_update_channel),
    exposed("channels", "delete_channel", "Exclui um canal de um projeto.", "Para excluir um canal de um projeto; ação destrutiva.", schema_delete_channel),
    exposed("backlog", "list_backlog", "Lista o backlog de um projeto.", "Para ver todos os cards de um projeto com as colunas; use projectId de list_projects.", empty_object_schema),
    exposed("backlog", "save_backlog_item", "Grava um item do backlog.", "Para gravar um card completo já existente; prefira create_backlog_item para criar.", empty_object_schema),
    exposed("backlog", "create_backlog_item", "Cria um card no backlog.", "Para criar um card no backlog de um projeto com título e descrição.", empty_object_schema),
    exposed("backlog", "create_request_from_proposal", "Cria um card de origem agente a partir de uma proposta.", "Para transformar uma proposta de um agente em card de origem agente; título repetido é recusado.", empty_object_schema),
    exposed("backlog", "move_backlog_item", "Move um item do backlog para outra coluna.", "Para mover um card de coluna; as colunas válidas são as do quadro, e Done só o usuário conclui.", empty_object_schema),
    exposed("backlog", "reorder_backlog_column", "Reordena os cards de uma coluna do backlog.", "Para definir a ordem dos cards dentro de uma coluna (projectId, column, itemIds).", empty_object_schema),
    exposed("backlog", "set_item_priority", "Define ou limpa a prioridade de um item do backlog.", "Para definir a prioridade de um card (projectId, itemId, priority: Baixa|Média|Alta|Urgente ou vazio para limpar).", empty_object_schema),
    exposed("backlog", "return_task", "Devolve uma task concluída ao líder com um motivo.", "Para devolver ao líder uma task concluída com um motivo obrigatório; o card volta a In progress.", empty_object_schema),
    exposed("backlog", "save_item_plan", "Grava o plano de um item do backlog (proposal no protocolo #plano-pronto com #lote, que é o formato recomendado; qualquer texto é aceito).", "Para gravar o plano de execução de um card.", schema_save_item_plan),
    exposed("backlog", "add_backlog_label", "Adiciona um rótulo a um item do backlog.", "Para marcar um card com um rótulo.", empty_object_schema),
    exposed("backlog", "remove_backlog_label", "Remove um rótulo de um item do backlog.", "Para tirar um rótulo de um card.", empty_object_schema),
    exposed("backlog", "link_git_ref", "Vincula um commit, branch ou PR a um card do backlog. Parâmetros: projectId, itemId, kind (commit|branch|pr), refId, label (opcional), url (opcional). Devolve o Link criado.", "Para associar um commit, branch ou PR a um card sem duplicar vínculos.", schema_link_git_ref),
    exposed("backlog", "unlink_git_ref", "Remove um vínculo Git de um card do backlog. Parâmetros: projectId, itemId, linkId.", "Para desvincular um commit, branch ou PR de um card pelo linkId (ex.: commit:abc1234).", schema_unlink_git_ref),
    exposed("backlog", "list_git_refs", "Lista branches e commits recentes do repositório Git do projeto. Parâmetros: projectId, limit (opcional, padrão 20, máx 100).", "Para consultar referências Git disponíveis no projeto antes de vincular a um card.", schema_list_git_refs),
    exposed("backlog", "set_backlog_responsible", "Define ou troca o agente responsável por um item do backlog.", "Para definir ou trocar o responsável de um card (projectId, itemId, agent); o agente precisa existir no elenco. Trocar um responsável já definido cabe ao Ember ou ao responsável atual; indicar o próprio responsável não faz nada.", empty_object_schema),
    exposed("backlog", "archive_backlog_item", "Arquiva um item do backlog.", "Para arquivar um card que saiu de uso sem apagá-lo.", empty_object_schema),
    exposed("backlog", "restore_backlog_item", "Restaura um item arquivado do backlog.", "Para trazer de volta um card arquivado.", empty_object_schema),
    exposed("backlog", "delete_backlog_item", "Exclui um item do backlog.", "Para excluir um card definitivamente; ação destrutiva.", empty_object_schema),
    exposed("artifacts", "list_artifact_kinds", "Lista os 5 tipos de artefato livre com rótulo, descrição e template Markdown.", "Para descobrir os tipos válidos e o template de cada um antes de criar um artefato.", empty_object_schema),
    exposed("artifacts", "list_artifacts", "Lista os artefatos de um projeto (planos e documentos). Parâmetro: projectId. Devolve id, kind, title, summary, tags, author, itemId e updatedAt.", "Para ver tudo o que existe de artefato num projeto, sem o corpo; leia um deles com get_artifact.", schema_project_id),
    exposed("artifacts", "get_artifact", "Lê um artefato por id. Parâmetros: projectId, id. Devolve o plano com critérios (id plano-*) ou o documento com o corpo em Markdown.", "Para ler o conteúdo completo de um artefato antes de editá-lo ou de usá-lo como contexto.", schema_artifact_ref),
    exposed("artifacts", "create_artifact", "Cria um artefato livre. kind: documento|especificacao|checklist|arquitetura|decisao; body em Markdown (sem body usa o template do tipo). Devolve o artefato com id <kind>-<n>.", "Para registrar uma especificação, decisão, checklist, arquitetura ou documento do projeto; consulte list_artifact_kinds para os templates.", schema_artifact_create),
    exposed("artifacts", "update_artifact", "Edita título, resumo, corpo ou tags de um artefato livre. Parâmetros: projectId, id e ao menos um campo; expectedUpdatedAt recusa a gravação se outro alterou antes. Planos (plano-*) são só leitura.", "Para corrigir ou completar um artefato existente; leia antes com get_artifact e envie expectedUpdatedAt para não sobrescrever uma edição alheia.", schema_artifact_update),
    exposed("artifacts", "delete_artifact", "Exclui um artefato livre ou um plano. Parâmetros: projectId, id. Plano (plano-*) é excluído com cascade: apaga também as etapas (cards plano-*.n) e os vínculos do card com o plano.", "Para remover um artefato ou plano que não serve mais; ação destrutiva e irreversível, confirme com o usuário.", schema_artifact_ref),
    exposed("artifacts", "set_artifact_check", "Marca ou desmarca um item de checklist do corpo de um artefato. Parâmetros: projectId, id, index (0 é o primeiro \"- [ ]\" fora de blocos de código), done. Devolve o artefato.", "Para acompanhar a entrega de um checklist item a item sem reescrever o corpo inteiro.", schema_artifact_check),
    exposed("artifacts", "set_artifact_criterion", "Marca ou desmarca um critério de aceite de um plano. Parâmetros: projectId, artifactId, criterionId, done.", "Para marcar um critério de aceite de um plano (artefato plano-*) como cumprido ou pendente.", schema_set_artifact_criterion),
    exposed("artifacts", "update_item_plan", "Reescreve o texto de um plano proposto ou aceito (protocolo #plano-pronto, #lote, #roteiro...). Parâmetros: projectId, itemId, proposal e expectedUpdatedAt opcional. Mantém os critérios já marcados e, no plano aceito, atualiza, cria ou arquiva as etapas. Devolve o plano.", "Para corrigir um plano sem refazê-lo: leia antes com get_artifact (id plano-<card>), envie o texto completo e o updatedAt lido em expectedUpdatedAt. Recusa remover etapa já iniciada.", schema_update_item_plan),
    exposed("artifacts", "import_plan", "Importa um plano pronto no protocolo do Vokia como um card novo com plano proposto. Parâmetros: projectId, title, proposal (qualquer texto; com #lote vira etapas) e author opcional. Devolve o card criado.", "Para trazer para o Vokia um plano já escrito (por exemplo, exportado de outro projeto). Texto livre é aceito como está; para ganhar etapas, escreva no protocolo do Vokia.", schema_import_plan),
    ui_only("artifacts", "save_plan_markdown", "Salva o plano em Markdown num arquivo escolhido pelo usuário."),
    ui_only("artifacts", "pick_plan_file", "Abre o seletor de arquivo para importar um plano em Markdown ou texto."),
    ui_only("attachments", "pick_attachment_files", "Abre o seletor de arquivos para anexos."),
    exposed("attachments", "save_attachment_files", "Salva arquivos locais como anexos de um projeto.", "Para anexar arquivos locais a um projeto a partir de caminhos no disco.", empty_object_schema),
    ui_only("attachments", "save_attachment_bytes", "Salva anexo enviado como bytes pela interface."),
    exposed("attachments", "discard_attachment", "Descarta um anexo de um projeto.", "Para remover um anexo que não será enviado.", empty_object_schema),
    exposed("attachments", "read_attachment_preview", "Lê uma imagem da pasta de anexos do Vokia e devolve como data URL (base64). Parâmetro: path.", "Para carregar a miniatura de uma imagem anexada validando que o caminho pertence à pasta de anexos.", schema_read_attachment_preview),
    exposed("agents", "list_agents", "Lista o elenco de agentes.", "Para ver o elenco completo de agentes com papel, provedor e modelo.", empty_object_schema),
    exposed("agents", "available_providers", "Lista os provedores instalados na máquina.", "Para saber quais provedores têm a CLI instalada na máquina antes de configurar um agente.", empty_object_schema),
    exposed("agents", "create_agent", "Cria um agente.", "Para criar um agente novo; o papel precisa existir (veja role_list) e o provedor, estar configurado.", schema_draft),
    exposed("agents", "update_agent", "Edita um agente.", "Para alterar nome, papel, provedor ou modelo de um agente existente.", schema_id_draft),
    exposed("agents", "delete_agent", "Exclui um agente.", "Para excluir um agente; os cards dele passam ao Ember. Confirme com o usuário.", schema_id),
    exposed("agents", "set_agent_instructions", "Define as instruções de um agente.", "Para trocar as instruções extras de um agente; a sessão dele é reiniciada.", schema_set_agent_instructions),
    exposed("agents", "list_temporary_overrides", "Lista as trocas temporárias de LLM.", "Para ver as trocas temporárias de LLM em vigor.", empty_object_schema),
    exposed("agents", "set_temporary_override", "Aplica uma troca temporária de LLM a um agente.", "Para forçar um agente a usar outro provedor ou modelo por um tempo, por exemplo ao bater limite de uso.", schema_set_temporary_override),
    exposed("agents", "clear_temporary_override", "Remove a troca temporária de LLM de um agente.", "Para devolver um agente ao provedor e modelo originais.", schema_agent),
    exposed("agents", "clear_all_temporary_overrides", "Remove todas as trocas temporárias de LLM.", "Para desfazer todas as trocas temporárias de LLM de uma vez.", empty_object_schema),
    exposed("agents", "update_ember_llm", "Atualiza o LLM do agente Ember.", "Para mudar o LLM do Ember e a lista de fallback; o provedor precisa suportar ferramentas.", schema_update_ember_llm),
    exposed("roles", "role_list", "Lista os papéis.", "Para ver os papéis disponíveis antes de criar ou editar um agente.", empty_object_schema),
    exposed("roles", "role_create", "Cria um papel.", "Para criar um papel novo com nome, descrição e instruções.", schema_role_create),
    exposed("roles", "role_update", "Edita um papel.", "Para editar um papel existente; precisa do id.", schema_id_edit),
    exposed("roles", "role_delete", "Exclui um papel.", "Para excluir um papel; recusado se algum agente ainda o usa.", schema_id),
    exposed("orchestration", "get_orchestration", "Devolve a orquestração (camadas, times e posições).", "Para ler camadas, times, líderes e posições da orquestração antes de alterá-la.", empty_object_schema),
    exposed("orchestration", "save_layer", "Grava uma camada da orquestração.", "Para criar ou editar uma camada da orquestração.", schema_draft),
    exposed("orchestration", "delete_layer", "Exclui uma camada da orquestração.", "Para excluir uma camada da orquestração.", schema_id),
    exposed("orchestration", "reorder_layers", "Reordena as camadas da orquestração.", "Para mudar a ordem das camadas; passe todos os ids na nova ordem.", schema_reorder_layers),
    exposed("orchestration", "save_team", "Grava um time da orquestração.", "Para criar ou editar um time, com líder opcional que precisa estar no elenco.", schema_save_team),
    exposed("orchestration", "delete_team", "Exclui um time da orquestração.", "Para excluir um time; os cards dos agentes dele passam ao Ember.", schema_id),
    exposed("orchestration", "place_agent", "Posiciona um agente na orquestração.", "Para posicionar um agente em uma camada ou time da orquestração.", schema_draft),
    exposed("orchestration", "unplace_agent", "Remove a posição de um agente na orquestração.", "Para tirar um agente da orquestração sem excluí-lo.", schema_unplace_agent),
    exposed("orchestration", "save_team_flow", "Grava o fluxo de um time.", "Para gravar de uma vez a estrutura do fluxo de um time (agentes, gateways e ligações); fluxo inválido volta com a lista de problemas.", schema_save_team_flow),
    exposed("orchestration", "set_final_validation", "Define o modo de validação final de um time.", "Para definir como o time faz a validação final de uma task.", schema_set_final_validation),
    exposed("orchestration", "set_judge_full_flow", "Liga ou desliga o julgamento de fluxo do líder de um time (args: teamId, enabled). Ligado, o líder escreve #fluxo completo|motivo ou #fluxo reduzido|motivo antes de delegar uma task nova, e o reduzido pula a camada de Revisão.", "Para deixar o líder do time escolher entre o fluxo completo e o reduzido em cada task.", schema_set_judge_full_flow),
    exposed("orchestration", "save_orchestration_settings", "Grava as configurações da orquestração.", "Para gravar os limites e opções gerais da orquestração.", schema_save_orchestration_settings),
    exposed("orchestration", "list_agent_activity", "Lista as execuções em andamento por agente.", "Para ver o que cada agente está executando agora em todos os projetos.", empty_object_schema),
    exposed("orchestration", "list_agent_copies", "Lista as réplicas de agentes em execução em um projeto.", "Para ver as réplicas de agentes em execução em um projeto e os cards delas.", schema_project_id),
    exposed("providers", "list_providers", "Lista os provedores configurados.", "Para ver os provedores configurados e se a CLI de cada um está instalada.", empty_object_schema),
    exposed("providers", "save_provider", "Grava a configuração de um provedor.", "Para criar ou atualizar a configuração de um provedor; o id usa letras, números, hífen ou sublinhado.", schema_save_provider),
    exposed("providers", "delete_provider", "Remove um provedor.", "Para remover um provedor; recusado para o claude e para provedores em uso por agentes.", schema_id),
    exposed("providers", "discover_provider_models", "Descobre os modelos disponíveis de um provedor.", "Para descobrir os modelos de um provedor antes de escolher um modelo para um agente.", schema_id),
    exposed("providers", "inspect_cli", "Inspeciona uma CLI e devolve seu texto de ajuda.", "Para ler o --help de uma CLI nova antes de configurá-la como provedor.", schema_inspect_cli),
    exposed("providers", "build_setup_prompt", "Monta o prompt para configurar uma CLI como provedor.", "Para montar o prompt que pede a um agente o JSON de configuração de uma CLI.", empty_object_schema),
    exposed("providers", "test_provider_config", "Testa a configuração de um provedor.", "Para testar uma configuração de provedor com uma chamada real antes de salvá-la.", schema_test_provider_config),
    exposed("providers", "auto_configure_provider", "Configura um provedor automaticamente com um agente.", "Para gerar a configuração de um provedor automaticamente a partir da ajuda da CLI.", empty_object_schema),
    exposed("settings", "db_problem", "Problema detectado ao abrir o banco, se houver.", "Para diagnosticar falha ao abrir o banco: devolve o problema, ou nada se estiver tudo certo.", empty_object_schema),
    exposed("settings", "erase_all_data", "Apaga todos os dados; exige { \"confirm\": true }.", "Para apagar todos os dados do Vokia; ação irreversível que só roda com confirm true, e só depois de o usuário pedir.", schema_erase_all_data),
    exposed("settings", "get_settings", "Devolve as configurações.", "Para ler as configurações gravadas antes de alterar alguma.", empty_object_schema),
    exposed("settings", "set_setting", "Grava uma configuração.", "Para gravar uma configuração por chave e valor; a chave active_llm troca o LLM padrão.", schema_set_setting),
    exposed("usage", "get_plan_usage", "Devolve o snapshot unificado de cotas de uso por provedor (janelas 5h, diária, semanal, mensal, grupos de modelos, limites ativos e retornos pendentes). Parâmetro opcional: providerId.", "Para consultar a cota atual de uso dos planos conectados, limites ativos por modelo e quando cada janela renova.", schema_plan_usage),
    exposed("usage", "refresh_plan_usage", "Atualiza e devolve as cotas de uso por provedor respeitando o UsageReadGate (single-flight, cache de 4 min e backoff exponencial em erro). Parâmetro opcional: providerId.", "Para atualizar a leitura de cotas dos provedores sob demanda sem exceder limites de requisição.", schema_plan_usage),
];

#[derive(Deserialize, Default)]
struct ConfirmArgs {
    #[serde(default)]
    confirm: bool,
}

// Sem chamador interno: o protocolo MCP usa a versão filtrada; mantida como API sem filtro.
#[cfg_attr(not(test), allow(dead_code))] // usado só nos testes
pub fn list_commands() -> Vec<Value> {
    list_commands_filtered(None)
}

pub fn command_domain(name: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .find(|command| command.name == name)
        .map(|command| command.domain)
}

pub fn list_commands_filtered(domain: Option<&str>) -> Vec<Value> {
    COMMANDS
        .iter()
        .filter(|command| command.exposure == Exposure::AppAndCli)
        .filter(|command| domain.is_none_or(|domain| command.domain == domain))
        .map(|command| {
            json!({
                "name": command.name,
                "domain": command.domain,
                "description": command.description,
                "when_to_use": command.when_to_use,
                "inputSchema": (command.schema)(),
            })
        })
        .collect()
}

// `caller` None é a interface do usuário, com permissão global; agentes passam o próprio id.
pub async fn dispatch(
    app: &AppHandle,
    caller: Option<&Agent>,
    name: &str,
    arguments: Value,
) -> Option<Result<Value, String>> {
    let canonical = name.strip_prefix(APP_PREFIX).unwrap_or(name);
    let command = COMMANDS.iter().find(|command| command.name == canonical)?;
    if command.exposure == Exposure::UiOnly {
        return Some(Err(UI_ONLY_ERROR.to_string()));
    }
    Some(run(app, caller, canonical, arguments).await)
}

fn parse_args<T: DeserializeOwned>(arguments: Value) -> Result<T, String> {
    let normalized = if arguments.is_null() {
        json!({})
    } else {
        arguments
    };
    serde_json::from_value(normalized).map_err(|error| format!("Argumentos inválidos: {error}"))
}

fn json_ok<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

fn json_result<T: Serialize, E: ToString>(result: Result<T, E>) -> Result<Value, String> {
    result.map_err(|error| error.to_string()).and_then(json_ok)
}

fn json_project_result<T: Serialize>(result: Result<T, ProjectError>) -> Result<Value, String> {
    json_result(result.map_err(|error| error.message().to_string()))
}

async fn run(
    app: &AppHandle,
    caller: Option<&Agent>,
    name: &str,
    arguments: Value,
) -> Result<Value, String> {
    match name {
        "cli_problem" => json_ok(team::cli_problem_exec(app)),
        "send_request" => json_result(team::send_request_exec(app, parse_args(arguments)?).await),
        "answer_permission" => {
            json_result(team::answer_permission_exec(app, parse_args(arguments)?))
        }
        "answer_decision" => {
            json_result(team::answer_decision_exec(app, parse_args(arguments)?).await)
        }
        "list_agent_pauses" => json_result(team::list_agent_pauses_exec(app)),
        "list_interrupted_runs" => json_result(team::list_interrupted_runs_exec(app)),
        "resume_interrupted_run" => {
            json_result(team::resume_interrupted_run_exec(app, parse_args(arguments)?).await)
        }
        "discard_interrupted_run" => json_result(team::discard_interrupted_run_exec(
            app,
            parse_args(arguments)?,
        )),
        "set_agent_paused" => json_result(team::set_agent_paused_exec(app, parse_args(arguments)?)),
        "list_queued_requests" => json_result(team::list_queued_requests_exec(app)),
        "enqueue_queued_request" => json_result(team::enqueue_queued_request_exec(
            app,
            parse_args(arguments)?,
        )),
        "update_queued_request" => json_result(team::update_queued_request_exec(
            app,
            parse_args(arguments)?,
        )),
        "move_queued_request" => json_result(team::move_queued_request_exec(
            app,
            parse_args(arguments)?,
        )),
        "remove_queued_request" => json_result(team::remove_queued_request_exec(
            app,
            parse_args(arguments)?,
        )),
        "check_connection" => json_ok(team::check_connection_exec().await),
        "agent_console" => json_ok(team::agent_console_exec(app, parse_args(arguments)?)),

        "list_projects" => json_result(projects::list_projects_exec(app)),
        "select_project" => json_result(projects::select_project_exec(app, parse_args(arguments)?)),
        "inspect_folder" => {
            json_project_result(projects::inspect_folder_exec(parse_args(arguments)?).await)
        }
        "inspect_github_repo" => {
            json_project_result(projects::inspect_github_repo_exec(parse_args(arguments)?).await)
        }
        "create_project" => {
            json_project_result(projects::create_project_exec(app, parse_args(arguments)?).await)
        }
        "update_project" => {
            json_project_result(projects::update_project_exec(app, parse_args(arguments)?))
        }
        "delete_project" => {
            json_project_result(projects::delete_project_exec(app, parse_args(arguments)?).await)
        }
        "pin_project" => {
            json_project_result(projects::pin_project_exec(app, parse_args(arguments)?))
        }
        "clear_conversation" => {
            json_result(projects::clear_conversation_exec(app, parse_args(arguments)?).await)
        }
        "load_history" => json_result(projects::load_history_exec(app, parse_args(arguments)?)),
        "set_permission_level" => {
            json_result(projects::set_permission_level_exec(app, parse_args(arguments)?).await)
        }
        "get_execution_mode" => {
            json_result(execution_mode::get_execution_mode_exec(app, parse_args(arguments)?))
        }
        "list_permission_rules" => json_result(projects::list_permission_rules_exec(
            app,
            parse_args(arguments)?,
        )),
        "delete_permission_rule" => json_result(projects::delete_permission_rule_exec(
            app,
            parse_args(arguments)?,
        )),

        "list_channels" => json_result(channels::list_channels_exec(app, parse_args(arguments)?)),
        "create_channel" => json_result(channels::create_channel_exec(app, parse_args(arguments)?)),
        "update_channel" => json_result(channels::update_channel_exec(app, parse_args(arguments)?)),
        "delete_channel" => json_result(channels::delete_channel_exec(app, parse_args(arguments)?)),

        "list_backlog" => json_result(backlog::list_backlog_exec(app, parse_args(arguments)?)),
        "save_backlog_item" => {
            json_result(backlog::save_backlog_item_exec(app, parse_args(arguments)?))
        }
        "create_backlog_item" => json_result(backlog::create_backlog_item_exec(
            app,
            parse_args(arguments)?,
        )),
        "create_request_from_proposal" => json_result(backlog::create_request_from_proposal_exec(
            app,
            parse_args(arguments)?,
        )),
        "move_backlog_item" => {
            json_result(backlog::move_backlog_item_exec(app, parse_args(arguments)?))
        }
        "reorder_backlog_column" => json_result(backlog::reorder_backlog_column_exec(
            app,
            parse_args(arguments)?,
        )),
        "set_item_priority" => json_result(backlog::set_item_priority_exec(
            app,
            parse_args(arguments)?,
        )),
        "return_task" => json_result(backlog::return_task_exec(app, parse_args(arguments)?).await),
        "save_item_plan" => json_result(backlog::save_item_plan_exec(app, parse_args(arguments)?)),
        "add_backlog_label" => {
            json_result(backlog::add_backlog_label_exec(app, parse_args(arguments)?))
        }
        "remove_backlog_label" => json_result(backlog::remove_backlog_label_exec(
            app,
            parse_args(arguments)?,
        )),
        "link_git_ref" => json_result(backlog::link_git_ref_exec(app, parse_args(arguments)?)),
        "unlink_git_ref" => json_result(backlog::unlink_git_ref_exec(app, parse_args(arguments)?)),
        "list_git_refs" => {
            json_result(backlog::list_git_refs_exec(app, parse_args(arguments)?).await)
        }
        "set_backlog_responsible" => json_result(backlog::set_backlog_responsible_exec(
            app,
            parse_args(arguments)?,
            caller,
        )),
        "archive_backlog_item" => json_result(backlog::archive_backlog_item_exec(
            app,
            parse_args(arguments)?,
        )),
        "restore_backlog_item" => json_result(backlog::restore_backlog_item_exec(
            app,
            parse_args(arguments)?,
        )),
        "delete_backlog_item" => json_result(backlog::delete_backlog_item_exec(
            app,
            parse_args(arguments)?,
        )),

        "list_artifact_kinds" => json_ok(artifacts::list_artifact_kinds_exec()),
        "list_artifacts" => json_result(artifacts::list_artifacts_exec(app, parse_args(arguments)?)),
        "get_artifact" => json_result(artifacts::get_artifact_exec(app, parse_args(arguments)?)),
        "create_artifact" => json_result(artifacts::create_artifact_exec(app, parse_args(arguments)?)),
        "update_artifact" => json_result(artifacts::update_artifact_exec(app, parse_args(arguments)?)),
        "delete_artifact" => json_result(artifacts::delete_artifact_exec(app, parse_args(arguments)?)),
        "set_artifact_check" => {
            json_result(artifacts::set_artifact_check_exec(app, parse_args(arguments)?))
        }
        "update_item_plan" => {
            json_result(artifacts::update_item_plan_exec(app, parse_args(arguments)?))
        }
        "import_plan" => json_result(artifacts::import_plan_exec(app, parse_args(arguments)?)),
        "set_artifact_criterion" => {
            json_result(artifacts::set_artifact_criterion_exec(app, parse_args(arguments)?))
        }

        "save_attachment_files" => {
            json_result(attachments::save_attachment_files_exec(app, parse_args(arguments)?).await)
        }
        "discard_attachment" => {
            json_result(attachments::discard_attachment_exec(app, parse_args(arguments)?).await)
        }
        "read_attachment_preview" => json_result(attachments::read_attachment_preview_exec(
            app,
            parse_args(arguments)?,
        )),

        "list_agents" => json_result(agents::list_agents_exec(app)),
        "available_providers" => json_ok(agents::available_providers_exec(app)),
        "create_agent" => json_result(agents::create_agent_exec(app, parse_args(arguments)?)),
        "update_agent" => json_result(agents::update_agent_exec(app, parse_args(arguments)?).await),
        "delete_agent" => json_result(agents::delete_agent_exec(app, parse_args(arguments)?).await),
        "set_agent_instructions" => {
            json_result(agents::set_agent_instructions_exec(app, parse_args(arguments)?).await)
        }
        "list_temporary_overrides" => json_ok(agents::list_temporary_overrides_exec(app)),
        "set_temporary_override" => {
            json_result(agents::set_temporary_override_exec(app, parse_args(arguments)?).await)
        }
        "clear_temporary_override" => {
            json_result(agents::clear_temporary_override_exec(app, parse_args(arguments)?).await)
        }
        "clear_all_temporary_overrides" => {
            json_result(agents::clear_all_temporary_overrides_exec(app).await)
        }
        "update_ember_llm" => {
            json_result(agents::update_ember_llm_exec(app, parse_args(arguments)?).await)
        }

        "role_list" => json_result(roles::role_list_exec(app)),
        "role_create" => json_result(roles::role_create_exec(app, parse_args(arguments)?)),
        "role_update" => json_result(roles::role_update_exec(app, parse_args(arguments)?)),
        "role_delete" => json_result(roles::role_delete_exec(app, parse_args(arguments)?)),

        "get_orchestration" => json_result(orchestration::get_orchestration_exec(app)),
        "save_layer" => json_result(orchestration::save_layer_exec(app, parse_args(arguments)?)),
        "delete_layer" => json_result(orchestration::delete_layer_exec(
            app,
            parse_args(arguments)?,
        )),
        "reorder_layers" => json_result(orchestration::reorder_layers_exec(
            app,
            parse_args(arguments)?,
        )),
        "save_team" => json_result(orchestration::save_team_exec(app, parse_args(arguments)?)),
        "delete_team" => json_result(orchestration::delete_team_exec(app, parse_args(arguments)?)),
        "place_agent" => json_result(orchestration::place_agent_exec(app, parse_args(arguments)?)),
        "unplace_agent" => json_result(orchestration::unplace_agent_exec(
            app,
            parse_args(arguments)?,
        )),
        "save_team_flow" => json_result(orchestration::save_team_flow_exec(
            app,
            parse_args(arguments)?,
        )),
        "set_final_validation" => json_result(orchestration::set_final_validation_exec(
            app,
            parse_args(arguments)?,
        )),
        "set_judge_full_flow" => json_result(orchestration::set_judge_full_flow_exec(
            app,
            parse_args(arguments)?,
        )),
        "save_orchestration_settings" => json_result(
            orchestration::save_orchestration_settings_exec(app, parse_args(arguments)?),
        ),
        "list_agent_activity" => json_result(orchestration::list_agent_activity_exec(app)),
        "list_agent_copies" => json_result(orchestration::list_agent_copies_exec(
            app,
            parse_args(arguments)?,
        )),

        "list_providers" => json_result(providers::list_providers_exec(app)),
        "save_provider" => json_result(providers::save_provider_exec(app, parse_args(arguments)?)),
        "delete_provider" => {
            json_result(providers::delete_provider_exec(app, parse_args(arguments)?))
        }
        "discover_provider_models" => {
            json_result(providers::discover_provider_models_exec(app, parse_args(arguments)?).await)
        }
        "inspect_cli" => json_result(providers::inspect_cli_exec(parse_args(arguments)?).await),
        "build_setup_prompt" => json_ok(providers::build_setup_prompt_exec(parse_args(arguments)?)),
        "test_provider_config" => {
            json_result(providers::test_provider_config_exec(parse_args(arguments)?).await)
        }
        "auto_configure_provider" => {
            json_result(providers::auto_configure_provider_exec(app, parse_args(arguments)?).await)
        }

        "db_problem" => json_ok(settings::db_problem_exec(app)),
        "erase_all_data" => {
            let args: ConfirmArgs = parse_args(arguments)?;
            json_result(settings::erase_all_data_exec(app, args.confirm).await)
        }
        "get_settings" => json_result(settings::get_settings_exec(app)),
        "set_setting" => json_result(settings::set_setting_exec(app, parse_args(arguments)?).await),

        "get_plan_usage" => {
            json_result(usage::get_plan_usage_exec(app, parse_args(arguments)?).await)
        }
        "refresh_plan_usage" => {
            json_result(usage::refresh_plan_usage_exec(app, parse_args(arguments)?).await)
        }

        _ => Err(format!("Comando sem despacho registrado: {name}")),
    }
}
