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
