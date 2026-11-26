import { findTrigger, type TriggerMatch } from './composerMentions'

export interface SlashCommand {
  id: 'plano' | 'revisar' | 'pausar' | 'lembrar' | 'setup' | 'triar' | 'iniciar' | 'btw'
  description: string
  enabled: boolean
  prompt?: string
  action?: 'setup' | 'triar' | 'iniciar' | 'btw' | 'pausar' | 'lembrar'
}

export const SLASH_COMMANDS: readonly SlashCommand[] = [
  {
    id: 'btw',
    description: 'pergunta rápida sem interromper nem entrar na fila',
    enabled: true,
    action: 'btw',
  },
  {
    id: 'plano',
    description: 'quebrar o pedido em lotes',
    enabled: true,
    prompt: 'Quebre este pedido em lotes, com tarefas curtas por lote: ',
  },
  {
    id: 'revisar',
    description: 'chamar o revisor agora',
    enabled: true,
    prompt: 'Faça agora a revisão do que foi feito até aqui. ',
  },
  { id: 'pausar', description: 'pausar ou retomar um agente ou o time', enabled: true, action: 'pausar' },
  { id: 'lembrar', description: 'salvar um fato na memória do projeto', enabled: true, action: 'lembrar' },
  { id: 'setup', description: 'configurar ou adicionar um provedor de LLM', enabled: true, action: 'setup' },
  { id: 'triar', description: 'iniciar triagem de um item do backlog', enabled: true, action: 'triar' },
  { id: 'iniciar', description: 'iniciar um item pronto do backlog', enabled: true, action: 'iniciar' },
]

export const COMMAND_TRIGGER = '/'

const COMMAND_PATTERN = /^\/(\S+)(?:\s+([\s\S]*))?$/

export function findCommandTrigger(text: string, caret: number): TriggerMatch | null {
  const match = findTrigger(text, caret, COMMAND_TRIGGER)
  return match?.start === 0 ? match : null
}

export function parseLocalCommand(
  text: string,
): { action: 'setup' | 'triar' | 'iniciar' | 'btw' | 'pausar' | 'lembrar'; arg: string } | null {
  const match = COMMAND_PATTERN.exec(text.trim())
  if (match === null) return null
  const cmd = match[1]
  if (
    cmd !== 'setup' &&
    cmd !== 'triar' &&
    cmd !== 'iniciar' &&
    cmd !== 'btw' &&
    cmd !== 'pausar' &&
    cmd !== 'lembrar'
  ) {
    return null
  }
  return { action: cmd, arg: (match[2] ?? '').trim() }
}

export function expandCommand(text: string): { text: string } | null {
  const match = COMMAND_PATTERN.exec(text.trim())
  if (match === null) {
    return null
  }
  const command = SLASH_COMMANDS.find((candidate) => candidate.id === match[1])
  if (command?.enabled !== true || command.prompt === undefined) {
    return null
  }
  return { text: `${command.prompt}${match[2] ?? ''}`.trimEnd() }
}
