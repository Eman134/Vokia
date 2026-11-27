use serde_json::{json, Value};

use crate::db::ARTIFACT_KINDS;
use crate::plan_proposal::PLAN_PROTOCOL_HINT;

pub(super) fn empty_object_schema() -> Value {
    json!({ "type": "object" })
}

pub(super) fn obj_schema(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

pub(super) fn schema_id() -> Value {
    obj_schema(json!({ "id": { "type": "string" } }), &["id"])
}

pub(super) fn schema_draft() -> Value {
    obj_schema(json!({ "draft": { "type": "object" } }), &["draft"])
}

pub(super) fn schema_agent() -> Value {
    obj_schema(json!({ "agent": { "type": "string" } }), &["agent"])
}

pub(super) fn schema_project_id() -> Value {
    obj_schema(
        json!({ "projectId": { "type": "integer" } }),
        &["projectId"],
    )
}

pub(super) fn schema_link_git_ref() -> Value {
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

pub(super) fn schema_unlink_git_ref() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "itemId": { "type": "string" },
            "linkId": { "type": "string" },
        }),
        &["projectId", "itemId", "linkId"],
    )
}

pub(super) fn schema_list_git_refs() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "limit": { "type": "integer" },
        }),
        &["projectId"],
    )
}

pub(super) fn schema_read_attachment_preview() -> Value {
    obj_schema(json!({ "path": { "type": "string" } }), &["path"])
}

pub(super) fn schema_role_create() -> Value {
    obj_schema(
        json!({ "draft": {
            "type": "object",
            "description": "Objeto RoleDraft (name, description, instructions)",
        } }),
        &["draft"],
    )
}

pub(super) fn schema_id_edit() -> Value {
    obj_schema(
        json!({ "id": { "type": "string" }, "edit": { "type": "object" } }),
        &["id", "edit"],
    )
}

pub(super) fn schema_id_draft() -> Value {
    obj_schema(
        json!({ "id": { "type": "string" }, "draft": { "type": "object" } }),
        &["id", "draft"],
    )
}

pub(super) fn schema_set_agent_instructions() -> Value {
    obj_schema(
        json!({ "id": { "type": "string" }, "text": { "type": "string" } }),
        &["id", "text"],
    )
}

pub(super) fn schema_set_temporary_override() -> Value {
    obj_schema(
        json!({
            "agent": { "type": "string" },
            "cli": { "type": "string" },
            "modelTier": { "type": "string" },
            "model": { "type": "string" },
            "reasoningEffort": { "type": "string", "description": "Esforço de raciocínio (low, medium, high, max)" },
            "reason": { "type": "string" },
            "expiresInMs": { "type": "integer" },
        }),
        &["agent", "cli"],
    )
}

pub(super) fn schema_update_ember_llm() -> Value {
    obj_schema(
        json!({
            "cli": { "type": "string" },
            "modelTier": { "type": "string" },
            "model": { "type": "string" },
            "reasoningEffort": { "type": "string", "description": "Esforço de raciocínio (low, medium, high, max)" },
            "fallback": { "type": "array" },
        }),
        &[],
    )
}

pub(super) fn schema_optional_id() -> Value {
    obj_schema(json!({ "id": { "type": "string" } }), &[])
}

pub(super) fn schema_set_active_llm() -> Value {
    obj_schema(
        json!({
            "cli": { "type": "string", "description": "ID do provedor registrado" },
            "modelTier": { "type": "string", "description": "basic | intermediate | advanced (opcional)" },
            "model": { "type": "string", "description": "ID explícito do modelo (opcional)" },
            "reasoningEffort": { "type": "string", "description": "Esforço de raciocínio (low, medium, high, max)" },
        }),
        &["cli"],
    )
}

pub(super) fn schema_set_provider_enabled() -> Value {
    obj_schema(
        json!({
            "id": { "type": "string", "description": "ID do provedor" },
            "enabled": { "type": "boolean", "description": "Define se o provedor está ativo para uso pelos agentes" },
            "reason": { "type": "string", "description": "Motivo opcional quando desativado" },
        }),
        &["id", "enabled"],
    )
}

pub(super) fn schema_reorder_layers() -> Value {
    obj_schema(
        json!({ "ids": { "type": "array", "items": { "type": "string" } } }),
        &["ids"],
    )
}

pub(super) fn schema_save_team() -> Value {
    obj_schema(
        json!({ "draft": { "type": "object" }, "leaderAgent": { "type": "string" } }),
        &["draft"],
    )
}

pub(super) fn schema_unplace_agent() -> Value {
    obj_schema(json!({ "agentId": { "type": "string" } }), &["agentId"])
}

pub(super) fn schema_save_team_flow() -> Value {
    obj_schema(
        json!({ "teamId": { "type": "string" }, "draft": { "type": "object" } }),
        &["teamId", "draft"],
    )
}

pub(super) fn schema_set_final_validation() -> Value {
    obj_schema(
        json!({ "teamId": { "type": "string" }, "mode": { "type": "string" } }),
        &["teamId", "mode"],
    )
}

pub(super) fn schema_set_judge_full_flow() -> Value {
    obj_schema(
        json!({ "teamId": { "type": "string" }, "enabled": { "type": "boolean" } }),
        &["teamId", "enabled"],
    )
}

pub(super) fn schema_save_orchestration_settings() -> Value {
    obj_schema(json!({ "settings": { "type": "object" } }), &["settings"])
}

pub(super) fn schema_save_provider() -> Value {
    obj_schema(json!({ "config": { "type": "object" } }), &["config"])
}

pub(super) fn schema_inspect_cli() -> Value {
    obj_schema(json!({ "command": { "type": "string" } }), &["command"])
}

pub(super) fn schema_test_provider_config() -> Value {
    obj_schema(
        json!({ "config": { "type": "object" }, "model": { "type": "string" } }),
        &["config"],
    )
}

pub(super) fn schema_create_channel() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "name": { "type": "string" },
            "members": { "type": "array", "items": { "type": "string" } },
        }),
        &["projectId", "name", "members"],
    )
}

pub(super) fn schema_update_channel() -> Value {
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

pub(super) fn schema_delete_channel() -> Value {
    obj_schema(
        json!({ "projectId": { "type": "integer" }, "id": { "type": "integer" } }),
        &["projectId", "id"],
    )
}

pub(super) fn schema_set_setting() -> Value {
    obj_schema(
        json!({ "key": { "type": "string" }, "value": { "type": "string" } }),
        &["key", "value"],
    )
}

pub(super) fn schema_artifact_ref() -> Value {
    obj_schema(
        json!({ "projectId": { "type": "integer" }, "id": { "type": "string" } }),
        &["projectId", "id"],
    )
}

pub(super) fn artifact_kind_names() -> Vec<&'static str> {
    ARTIFACT_KINDS.iter().map(|kind| kind.kind).collect()
}

pub(super) fn schema_artifact_create() -> Value {
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

pub(super) fn schema_artifact_update() -> Value {
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

pub(super) fn schema_artifact_check() -> Value {
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

pub(super) fn schema_set_artifact_criterion() -> Value {
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

pub(super) fn schema_web_search() -> Value {
    obj_schema(
        json!({
            "query": { "type": "string", "description": "Termo ou pergunta de pesquisa na web" },
            "projectId": { "type": "integer", "description": "ID do projeto (opcional; no MCP usa o projeto vinculado)" },
        }),
        &["query"],
    )
}

pub(super) fn schema_save_item_plan() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer" },
            "itemId": { "type": "string" },
            "plan": {
                "type": "object",
                "properties": {
                    "state": { "type": "string", "description": "vazio | proposto | vinculado" },
                    "publicId": { "type": "string", "description": "Identificador público curto do card (ex: FIL01, ANIM01)" },
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

pub(super) fn schema_update_item_plan() -> Value {
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

pub(super) fn schema_import_plan() -> Value {
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

pub(super) fn schema_erase_all_data() -> Value {
    obj_schema(json!({ "confirm": { "type": "boolean" } }), &["confirm"])
}

pub(super) fn schema_ask_btw() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer", "description": "Id opcional do projeto" },
            "agent": { "type": "string", "description": "Id opcional do agente alvo" },
            "question": { "type": "string", "description": "Pergunta rápida paralela (/btw)" },
        }),
        &["question"],
    )
}

pub(super) fn schema_plan_usage() -> Value {
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

pub(super) fn schema_get_turn_plan() -> Value {
    obj_schema(
        json!({
            "agent": { "type": "string", "description": "Id do agente (ex.: hefesto, ember)" },
            "projectId": { "type": "integer", "description": "Id opcional do projeto; se omitido usa o projeto ativo ou o último plano registrado do agente" },
            "cardId": { "type": "string", "description": "Id opcional do card do backlog" },
        }),
        &["agent"],
    )
}

pub(super) fn schema_get_usage_analytics() -> Value {
    obj_schema(
        json!({
            "days": { "type": "integer", "minimum": 1, "maximum": 367, "description": "Janela em dias (ex.: 7, 14, 30, 90; máximo 367)" },
            "from": { "type": "string", "description": "Data inicial YYYY-MM-DD ou RFC3339 (máximo 367 dias até to)" },
            "to": { "type": "string", "description": "Data final YYYY-MM-DD ou RFC3339" },
            "timezone": { "type": "string", "description": "Fuso IANA (ex.: America/Sao_Paulo), UTC ou deslocamento (±HH:MM)" },
            "projectId": { "type": "integer", "description": "Filtrar por projeto" },
            "agentId": { "type": "string", "description": "Filtrar por agente" },
            "providerId": { "type": "string", "description": "Filtrar por provedor" },
        }),
        &[],
    )
}

pub(super) fn schema_global_search() -> Value {
    obj_schema(
        json!({
            "query": { "type": "string", "description": "Termo ou frase para busca full-text (FTS5)" },
            "projectId": { "type": "integer", "description": "Filtrar resultados para um projeto específico" },
            "kinds": {
                "type": "array",
                "items": {
                    "type": "string",
                    "enum": ["projects", "agents", "cards", "plans", "artifacts", "memories", "messages"]
                },
                "description": "Tipos de entidade a incluir na busca"
            },
            "limit": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Limite de resultados por grupo (padrão 20)" },
            "cursor": { "type": "string", "description": "Cursor de paginação devolvido em nextCursor" },
            "offset": { "type": "integer", "minimum": 0, "description": "Deslocamento opcional quando cursor não é informado" },
        }),
        &["query"],
    )
}

pub(super) fn schema_command_search() -> Value {
    obj_schema(
        json!({
            "query": { "type": "string", "description": "Consulta por nome ou palavras-chave do comando (ex.: \"create artifact\")" },
            "queries": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Lista opcional de consultas alternativas executadas em uma única chamada"
            },
            "domain": { "type": "string", "description": "Filtro opcional por domínio do catálogo" },
            "limit": { "type": "integer", "minimum": 1, "maximum": 10, "description": "Quantidade de resultados (padrão 5, máximo 10)" },
        }),
        &[],
    )
}

pub(super) fn schema_history_search() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer", "description": "Id do projeto (obrigatório fora de sessão MCP; no MCP usa o projeto vinculado e recusa outro)" },
            "query": { "type": "string", "description": "Termo ou frase para buscar no histórico local da conversa" },
            "agent": { "type": "string", "description": "Filtro opcional por agente remetente ou destinatário" },
            "channelId": { "type": "integer", "description": "Filtro opcional por canal do projeto" },
            "cardId": { "type": "string", "description": "Filtro opcional por card do backlog (também aceita itemId)" },
            "limit": { "type": "integer", "minimum": 1, "maximum": 10, "description": "Limite de resultados (padrão 5, máximo 10)" },
        }),
        &["query"],
    )
}

pub(super) fn schema_history_read() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer", "description": "Id do projeto (obrigatório fora de sessão MCP; no MCP usa o projeto vinculado e recusa outro)" },
            "messageId": { "type": "integer", "description": "Id específico da mensagem a ler (suporta offset/nextOffset para textos longos)" },
            "beforeMessageId": { "type": "integer", "description": "Lê página anterior a este messageId" },
            "cursor": { "type": "string", "description": "Cursor devolvido em nextCursor (equivalente a beforeMessageId)" },
            "limit": { "type": "integer", "minimum": 1, "maximum": 20, "description": "Quantidade de mensagens por página (padrão 10, máximo 20)" },
            "offset": { "type": "integer", "minimum": 0, "description": "Deslocamento em caracteres para continuar uma mensagem longa" },
            "agent": { "type": "string", "description": "Filtro opcional por agente e chave de [work steps]" },
            "channelId": { "type": "integer", "description": "Filtro opcional por canal do projeto" },
            "cardId": { "type": "string", "description": "Filtro opcional por card do backlog (também aceita itemId)" },
            "includeWorkSteps": { "type": "boolean", "description": "Quando true, anexa blocos [work steps] registrados no banco" },
            "workStepsOffset": { "type": "integer", "minimum": 0, "description": "Offset de paginação de [work steps] devolvido em nextWorkStepsOffset" },
        }),
        &[],
    )
}

pub(super) fn schema_list_turn_work_steps() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer", "description": "Id do projeto (obrigatório fora de sessão MCP; no MCP usa o projeto vinculado e recusa outro)" },
            "agent": { "type": "string", "description": "Filtro opcional por agente" },
            "cardId": { "type": "string", "description": "Filtro opcional por card do backlog (também aceita itemId)" },
            "limit": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Quantidade máxima de blocos por página (padrão 10, máximo 50)" },
            "offset": { "type": "integer", "minimum": 0, "description": "Offset de paginação devolvido em nextOffset" },
        }),
        &[],
    )
}

pub(super) fn schema_run_memory_review() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer", "description": "Id do projeto" },
            "agent": { "type": "string", "description": "Agente opcional associado à revisão" },
            "cardId": { "type": "string", "description": "Card opcional associado à revisão" },
            "trigger": { "type": "string", "description": "Gatilho opcional (manual, card-done, turn-cadence, idle, clear-conversation)" },
        }),
        &["projectId"],
    )
}

pub(super) fn schema_list_learning_suggestions() -> Value {
    obj_schema(
        json!({
            "projectId": { "type": "integer", "description": "Filtro opcional por projeto" },
            "status": { "type": "string", "enum": ["pending", "applied", "dismissed"], "description": "Filtro opcional por estado" },
        }),
        &[],
    )
}

pub(super) fn schema_apply_learning_suggestion() -> Value {
    obj_schema(
        json!({
            "id": { "type": "string", "description": "Id da sugestão de aprendizado" },
            "scope": { "type": "string", "enum": ["project", "global"], "description": "Escopo opcional para gravação de SKILL.md (project ou global)" },
        }),
        &["id"],
    )
}

