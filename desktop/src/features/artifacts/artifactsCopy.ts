import type { ArtifactGroupId, StateFilter } from './artifactBrowse'
import type { ArtifactsLayout } from './artifactsView'

const MS_PER_MINUTE = 60_000
const MINUTES_PER_HOUR = 60
const HOURS_PER_DAY = 24

export const ARTIFACTS_EMPTY_TITLE = 'Nenhum artefato'
export const ARTIFACTS_LOADING_TEXT = 'Carregando os artefatos…'
export const ARTIFACTS_BACK_LABEL = 'Voltar'
export const ARTIFACTS_NEW_LABEL = 'Novo artefato'
export const ARTIFACTS_FILTER_LABEL = 'Tipo de artefato'
export const ARTIFACTS_ALL_LABEL = 'Todos'
export const ARTIFACTS_PLANS_LABEL = 'Planos'
export const ARTIFACTS_OPENING_TEXT = 'Abrindo o artefato…'

export const EDITOR_CREATE_TITLE = 'Novo artefato'
export const EDITOR_EDIT_TITLE = 'Editar artefato'
export const EDITOR_KIND_LABEL = 'Tipo'
export const EDITOR_TITLE_LABEL = 'Título'
export const EDITOR_TITLE_PLACEHOLDER = 'Dê um nome ao artefato'
export const EDITOR_SUMMARY_LABEL = 'Resumo'
export const EDITOR_SUMMARY_PLACEHOLDER = 'Uma ou duas frases (opcional)'
export const EDITOR_TAGS_LABEL = 'Etiquetas'
export const EDITOR_TAGS_PLACEHOLDER = 'separe com vírgula (opcional)'
export const EDITOR_BODY_LABEL = 'Corpo'
export const EDITOR_BODY_MODE_LABEL = 'Modo do corpo'
export const EDITOR_WRITE_LABEL = 'Escrever'
export const EDITOR_PREVIEW_LABEL = 'Pré-visualizar'
export const EDITOR_PREVIEW_EMPTY = 'Nada para mostrar ainda.'
export const EDITOR_CANCEL_LABEL = 'Cancelar'
export const EDITOR_CREATE_LABEL = 'Criar artefato'
export const EDITOR_SAVE_LABEL = 'Salvar'
export const EDITOR_SAVING_LABEL = 'Salvando…'
export const EDITOR_DISCARD_MESSAGE = 'Descartar as alterações deste artefato?'
export const EDITOR_KEEP_LABEL = 'Continuar editando'
export const EDITOR_DISCARD_LABEL = 'Descartar'
export const EDITOR_CONFLICT_MESSAGE = 'Este artefato foi alterado por outra pessoa ou agente depois que você abriu a edição. Recarregue para ver a versão atual.'
export const EDITOR_RELOAD_LABEL = 'Recarregar'
export const EDITOR_TITLE_MAX_CHARS = 200

export const DOCUMENT_EDIT_LABEL = 'Editar'
export const DOCUMENT_DELETE_LABEL = 'Excluir'
export const DOCUMENT_DELETE_MESSAGE = 'Excluir este artefato? Não dá para desfazer.'
export const DOCUMENT_DELETE_CONFIRM_LABEL = 'Excluir artefato'
export const DOCUMENT_ANONYMOUS_AUTHOR = 'Você'
export const DOCUMENT_EMPTY_BODY = 'Este artefato não tem conteúdo.'

export const PLAN_DOWNLOAD_LABEL = 'Baixar'
export const PLAN_DOWNLOAD_ARIA = 'Baixar plano em Markdown'
export const PLAN_EDIT_LABEL = 'Editar plano'
export const PLAN_STAGES_TITLE = 'Etapas'
export const PLAN_CRITERIA_TITLE = 'Critérios do plano'
export const PLAN_CARD_CRITERIA_TITLE = 'Critérios do card'
export const PLAN_RISKS_TITLE = 'Riscos'
export const PLAN_LOADING_TEXT = 'Carregando o plano…'
export const PLAN_RETRY_LABEL = 'Tentar de novo'
export const PLAN_SAVED_NOTICE = (path: string): string => `Plano salvo em ${path}`

export const STAGE_COLUMN_LABEL = 'Coluna da etapa'
export const STAGE_OPEN_LABEL = 'Abrir card'

export const PLAN_EDITOR_TITLE = 'Editar plano'
export const PLAN_EDITOR_SUMMARY_LABEL = 'Resumo'
export const PLAN_EDITOR_ESTIMATE_LABEL = 'Estimativa'
export const PLAN_EDITOR_ESTIMATE_PLACEHOLDER = '~2 h'
export const PLAN_EDITOR_RISK_LABEL = 'Riscos'
export const PLAN_EDITOR_STAGES_LABEL = 'Etapas'
export const PLAN_EDITOR_STAGE_TITLE_LABEL = 'Título'
export const PLAN_EDITOR_STAGE_GOAL_LABEL = 'Meta'
export const PLAN_EDITOR_STAGE_TASKS_LABEL = 'Tarefas (uma por linha)'
export const PLAN_EDITOR_SCRIPT_LABEL = 'Roteiro'
export const PLAN_EDITOR_WHO_LABEL = 'Quem'
export const PLAN_EDITOR_WHO_NONE = 'Sem responsável'
export const PLAN_EDITOR_CONTEXT_LABEL = 'Contexto'
export const PLAN_EDITOR_FILES_LABEL = 'Arquivos (separe com vírgula)'
export const PLAN_EDITOR_STEPS_LABEL = 'Passos (um por linha)'
export const PLAN_EDITOR_DONE_LABEL = 'Pronto quando (um por linha)'
export const PLAN_EDITOR_LIMITS_LABEL = 'Não fazer (um por linha)'
export const PLAN_EDITOR_ADD_STAGE_LABEL = 'Adicionar etapa'
export const PLAN_EDITOR_REMOVE_STAGE_LABEL = 'Remover'
export const PLAN_EDITOR_MOVE_UP_LABEL = 'Subir etapa'
export const PLAN_EDITOR_MOVE_DOWN_LABEL = 'Descer etapa'
export const PLAN_EDITOR_STARTED_HINT = 'Esta etapa já começou e não pode ser removida do plano.'
export const PLAN_EDITOR_EMPTY_STAGE_ERROR = 'Toda etapa precisa de um título.'
export const PLAN_EDITOR_NO_STAGES_ERROR = 'O plano precisa de pelo menos uma etapa.'
export const PLAN_EDITOR_DISCARD_MESSAGE = 'Descartar as alterações deste plano?'
export const PLAN_EDITOR_CONFLICT_MESSAGE = 'Este plano foi alterado por outra pessoa ou agente depois que você abriu a edição. Recarregue para ver a versão atual.'

export const IMPORT_LABEL = 'Importar plano'
export const IMPORT_TITLE = 'Importar plano'
export const IMPORT_SOURCE_LABEL = 'Origem do plano'
export const IMPORT_SOURCE_FILE = 'Arquivo'
export const IMPORT_SOURCE_PASTE = 'Colar texto'
export const IMPORT_PICK_LABEL = 'Escolher arquivo'
export const IMPORT_NO_FILE_TEXT = 'Nenhum arquivo escolhido.'
export const IMPORT_PASTE_LABEL = 'Texto do plano'
export const IMPORT_PASTE_PLACEHOLDER = 'Cole aqui o plano em Markdown ou texto'
export const IMPORT_TITLE_LABEL = 'Título'
export const IMPORT_CONTINUE_LABEL = 'Continuar'
export const IMPORT_BACK_LABEL = 'Voltar'
export const IMPORT_RECOGNIZED_TEXT = 'Plano do Vokia reconhecido'
export const IMPORT_SAVE_LABEL = 'Salvar plano'
export const IMPORT_FREE_TEXT = 'Texto livre: um agente vai organizar em etapas'
export const IMPORT_AGENT_LABEL = 'Quem organiza'
export const IMPORT_TRIAGE_LABEL = (agent: string): string => `Triar com ${agent}`
export const IMPORT_ANSWER_TEXT = 'O agente fez uma pergunta.'
export const IMPORT_ANSWER_LABEL = 'Responda no card'
export const IMPORT_OPEN_PLAN_LABEL = 'Abrir plano'
export const IMPORT_ACCEPT_LABEL = 'Aceitar e criar etapas'
export const IMPORT_CANCEL_LABEL = 'Cancelar'
export const IMPORT_KEEP_CARD_LABEL = 'Manter card'
export const IMPORT_DELETE_CARD_LABEL = 'Apagar card'
export const IMPORT_DELETE_CARD_MESSAGE = 'A triagem foi cancelada. Apagar o card criado para este plano?'
export const IMPORT_DELETE_CARD_CONFIRM = 'Apagar card'
export const IMPORT_DELETE_CARD_WARNING = 'O card some de vez. Não dá para desfazer.'

export function updatedAgo(updatedAt: number, now: number = Date.now()): string {
  const minutes = Math.max(0, Math.floor((now - updatedAt) / MS_PER_MINUTE))
  if (minutes < 1) return 'atualizado agora'
  if (minutes < MINUTES_PER_HOUR) return `atualizado há ${minutes} min`
  const hours = Math.floor(minutes / MINUTES_PER_HOUR)
  if (hours < HOURS_PER_DAY) return `atualizado há ${hours} h`
  return `atualizado há ${Math.floor(hours / HOURS_PER_DAY)} d`
}

// O backend devolve o rótulo no singular; a aba conta vários. Tipo novo sem plural aqui usa o rótulo como veio.
const KIND_PLURAL: Record<string, string> = {
  documento: 'Documentos',
  especificacao: 'Especificações',
  checklist: 'Checklists',
  arquitetura: 'Arquiteturas',
  decisao: 'Decisões',
}

export function kindTabLabel(kind: string, label: string): string {
  return Object.hasOwn(KIND_PLURAL, kind) ? KIND_PLURAL[kind] : label
}

const HEADING_LINE = /^#{1,6}\s/
const INLINE_HEADING = /(^|\s)#{1,6}\s+/g
const LIST_MARKER = /^(?:[-*+]|\d+\.)\s+/
const EMPHASIS = /\*\*|__/g
const CODE_TICKS = /`+/g
const SPACES = /\s+/g

// Só "**", "__" e crases saem: um "_" sozinho costuma ser parte de identificador (run_turns).
export function plainSummary(text: string): string {
  const lines = text.split('\n').map((line) => line.trim()).filter(Boolean)
  const body = lines.length > 1 ? lines.filter((line) => !HEADING_LINE.test(line)) : lines
  return (body.length > 0 ? body : lines)
    .map((line) => line.replace(LIST_MARKER, ''))
    .join(' ')
    .replace(INLINE_HEADING, '$1')
    .replace(EMPHASIS, '')
    .replace(CODE_TICKS, '')
    .replace(SPACES, ' ')
    .trim()
}

export const GROUP_LABEL: Record<ArtifactGroupId, string> = {
  precisa: 'Precisa de você',
  execucao: 'Em execução',
  prontos: 'Prontos',
  concluidos: 'Concluídos',
  documentos: 'Documentos',
}
export const STATE_FILTER_LABEL = 'Estado'
export const STATE_FILTER_OPTIONS: readonly { value: StateFilter; label: string }[] = [
  { value: 'todos', label: 'Todos os estados' },
  { value: 'proposta', label: 'Proposta' },
  { value: 'pronto', label: 'Pronto' },
  { value: 'execucao', label: 'Em execução' },
  { value: 'concluido', label: 'Concluído' },
]
export const SEARCH_LABEL = 'Buscar artefatos'
export const SEARCH_PLACEHOLDER = 'Buscar por título, ID ou resumo'
export const SEARCH_CLEAR_LABEL = 'Limpar busca'
export const NOTHING_FOUND_TITLE = 'Nada encontrado'
export const CLEAR_FILTERS_LABEL = 'Limpar filtros'
export const LAYOUT_LABEL = 'Visualização'
export const LAYOUT_OPTIONS: readonly { value: ArtifactsLayout; label: string }[] = [
  { value: 'grade', label: 'Grade' },
  { value: 'lista', label: 'Lista' },
]
export const PLAN_KIND_LABEL = 'Plano'
