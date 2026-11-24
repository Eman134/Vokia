import type { ActivityRowModel, AgentNameOf } from './activitySummary'
import type { FeedPermissionResult, PermissionResultOutcome, VokiaPermission } from './events'

interface OutcomeLine {
  verb: string
  failed: boolean
}

const OUTCOME_LINES: Readonly<Record<PermissionResultOutcome, OutcomeLine>> = {
  allowed: { verb: 'Aprovado', failed: false },
  denied: { verb: 'Negado', failed: true },
  expired: { verb: 'Expirou', failed: true },
  'project-rule': { verb: 'Regra do projeto', failed: false },
  auto: { verb: 'Modo auto', failed: false },
  bypass: { verb: 'Modo bypass', failed: false },
}

const PENDING_ROW_ID = 'permission-pending'
// Resultados que ninguém precisou decidir: entram recolhidos na conversa.
const AUTOMATIC_OUTCOMES: ReadonlySet<PermissionResultOutcome> = new Set(['auto', 'bypass', 'project-rule'])
const COMMAND_WORDS = 2
const SENTENCE_COMMANDS = 3
const MORE_COMMANDS = '…'

function commandChip(detail: string): ActivityRowModel['chip'] {
  return { label: detail, title: detail, icon: 'generic' }
}

export function permissionResultRow(result: FeedPermissionResult): ActivityRowModel {
  const { verb, failed } = OUTCOME_LINES[result.outcome]
  return {
    id: `permission-result-${result.time}`,
    icon: failed ? 'warning' : 'check',
    verb,
    chip: commandChip(result.detail),
    failed,
  }
}

export function permissionRequestRow(permission: VokiaPermission, nameOf: AgentNameOf): ActivityRowModel {
  return {
    id: `${PENDING_ROW_ID}-${permission.id}`,
    icon: 'command',
    verb: `${nameOf(permission.agent) ?? permission.agent} quer:`,
    chip: commandChip(permission.detail),
    meta: permission.tool,
    failed: false,
  }
}

export function isAutomaticPermission(result: FeedPermissionResult): boolean {
  return AUTOMATIC_OUTCOMES.has(result.outcome)
}

// "git commit -m ..." vira "git commit": o programa e o subcomando bastam para reconhecer o passo.
function commandHead(detail: string): string {
  return detail.trim().split(/\s+/).slice(0, COMMAND_WORDS).join(' ')
}

export function autoPermissionSentence(agentName: string, results: readonly FeedPermissionResult[]): string {
  const heads = [...new Set(results.map((result) => commandHead(result.detail)))]
  const shown = heads.slice(0, SENTENCE_COMMANDS)
  if (heads.length > SENTENCE_COMMANDS) shown.push(MORE_COMMANDS)
  const noun = results.length === 1 ? 'comando' : 'comandos'
  return `${agentName} rodou ${results.length} ${noun} (${shown.join(', ')})`
}
