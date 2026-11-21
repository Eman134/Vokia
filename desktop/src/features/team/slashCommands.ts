import { findTrigger, type TriggerMatch } from './composerMentions'

export interface SlashCommand {
  id: 'plano' | 'revisar' | 'pausar' | 'lembrar' | 'setup'
  description: string
  enabled: boolean
  prompt?: string
  action?: 'setup'
}

export const SLASH_COMMANDS: readonly SlashCommand[] = [
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
  { id: 'pausar', description: 'pausar um agente ou o time', enabled: false },
  { id: 'lembrar', description: 'salvar um fato na memória', enabled: false },
  { id: 'setup', description: 'configurar ou adicionar um provedor de LLM', enabled: true, action: 'setup' },
]

export const COMMAND_TRIGGER = '/'

const COMMAND_PATTERN = /^\/(\S+)(?:\s+([\s\S]*))?$/

export function findCommandTrigger(text: string, caret: number): TriggerMatch | null {
  const match = findTrigger(text, caret, COMMAND_TRIGGER)
  return match?.start === 0 ? match : null
}

export function parseLocalCommand(text: string): { action: 'setup'; arg: string } | null {
  const match = COMMAND_PATTERN.exec(text.trim())
  if (match === null || match[1] !== 'setup') {
    return null
  }
  return { action: 'setup', arg: (match[2] ?? '').trim() }
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
