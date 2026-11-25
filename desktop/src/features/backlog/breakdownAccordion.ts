import type { BreakdownStep } from './planBreakdown'

// Estados que as etapas herdam do card: repetir o selo em cada uma só adiciona ruído.
const INHERITED_STATUSES: readonly BreakdownStep['status'][] = ['proposta', 'pronto']
const DONE_STATUS: BreakdownStep['status'] = 'concluído'

export function firstPendingStepId(steps: readonly Pick<BreakdownStep, 'id' | 'status'>[]): string | undefined {
  return (steps.find((step) => step.status !== DONE_STATUS) ?? steps[0])?.id
}

// Uma etapa aberta por vez; clicar na aberta fecha.
export function toggleStep(expanded: ReadonlySet<string>, stepId: string): Set<string> {
  return expanded.has(stepId) ? new Set() : new Set([stepId])
}

export function showsStepStatus(step: Pick<BreakdownStep, 'status' | 'flowState'>): boolean {
  return step.flowState !== undefined || !INHERITED_STATUSES.includes(step.status)
}
