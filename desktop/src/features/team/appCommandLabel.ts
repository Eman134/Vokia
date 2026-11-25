import type { ToolStatus } from './events'

interface Phrases {
  running: string
  done: string
  failed: string
}

const TOKEN_SEPARATOR = '_'
const COMMAND_TITLE_MAX_CHARS = 80
const FALLBACK_PHRASES: Phrases = { running: 'Usando comando', done: 'Usou comando', failed: 'Tentou usar comando' }

const ACTION_VERBS: Readonly<Record<string, Phrases>> = {
  create: { running: 'Criando', done: 'Criou', failed: 'Tentou criar' },
  update: { running: 'Atualizando', done: 'Atualizou', failed: 'Tentou atualizar' },
  delete: { running: 'Removendo', done: 'Removeu', failed: 'Tentou remover' },
  save: { running: 'Salvando', done: 'Salvou', failed: 'Tentou salvar' },
  set: { running: 'Definindo', done: 'Definiu', failed: 'Tentou definir' },
  list: { running: 'Consultando', done: 'Consultou', failed: 'Tentou consultar' },
  get: { running: 'Consultando', done: 'Consultou', failed: 'Tentou consultar' },
}

const ENTITY_NOUNS: Readonly<Record<string, string>> = {
  role: 'o papel',
  agent: 'o agente',
  team: 'o time',
  layer: 'a camada',
  channel: 'o canal',
  project: 'o projeto',
  provider: 'o provedor',
  setting: 'a configuração',
  backlog_item: 'o card',
  item_plan: 'o plano',
  artifact: 'o artefato',
}

// Nomes sem o padrão ação_entidade e os comandos do domínio team.
const COMMAND_OVERRIDES: Readonly<Record<string, Phrases>> = {
  set_agent_instructions: {
    running: 'Definindo instruções do agente',
    done: 'Definiu instruções do agente',
    failed: 'Tentou definir instruções do agente',
  },
  send_request: { running: 'Enviando pedido', done: 'Enviou pedido', failed: 'Tentou enviar pedido' },
  answer_decision: { running: 'Respondendo decisão', done: 'Respondeu decisão', failed: 'Tentou responder decisão' },
  place_agent: { running: 'Posicionando o agente', done: 'Posicionou o agente', failed: 'Tentou posicionar o agente' },
  reorder_layers: {
    running: 'Reordenando as camadas',
    done: 'Reordenou as camadas',
    failed: 'Tentou reordenar as camadas',
  },
  global_search: {
    running: 'Buscando no workspace',
    done: 'Buscou no workspace',
    failed: 'Tentou buscar no workspace',
  },
}

// Ação e entidade valem em qualquer ordem dos tokens (create_agent e role_create); sem as duas, não há regra.
function phrasesOf(command: string): Phrases {
  if (Object.hasOwn(COMMAND_OVERRIDES, command)) return COMMAND_OVERRIDES[command]
  const tokens = command.split(TOKEN_SEPARATOR)
  const actionIndex = tokens.findIndex((token) => Object.hasOwn(ACTION_VERBS, token))
  if (actionIndex === -1) return FALLBACK_PHRASES
  const entity = tokens.filter((_, index) => index !== actionIndex).join(TOKEN_SEPARATOR)
  if (!Object.hasOwn(ENTITY_NOUNS, entity)) return FALLBACK_PHRASES
  const noun = ENTITY_NOUNS[entity]
  const verbs = ACTION_VERBS[tokens[actionIndex]]
  return {
    running: `${verbs.running} ${noun}`,
    done: `${verbs.done} ${noun}`,
    failed: `${verbs.failed} ${noun}`,
  }
}

export function commandPhrase(command: string, status: ToolStatus): string {
  return phrasesOf(command)[status === 'ok' ? 'done' : status]
}

export function friendlyCommand(command: string): string {
  return command.slice(0, COMMAND_TITLE_MAX_CHARS)
}
