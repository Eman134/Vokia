export const PLAN_LINE_SEPARATOR = ' · '

function sameText(a: string, b: string): boolean {
  return a.trim().toLocaleLowerCase('pt-BR') === b.trim().toLocaleLowerCase('pt-BR')
}

// O plano aceito costuma levar o título do card: repetir o nome logo abaixo do título só ocupa espaço.
export function linkedPlanLine(title: string, plan: { name: string; meta: string }): string | undefined {
  const parts = [sameText(plan.name, title) ? '' : plan.name.trim(), plan.meta.trim()].filter(Boolean)
  return parts.length > 0 ? parts.join(PLAN_LINE_SEPARATOR) : undefined
}
