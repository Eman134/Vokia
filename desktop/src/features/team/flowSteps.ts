export type FlowId = 'onboarding' | 'novo-projeto'

export const FLOW_STEPS: Record<FlowId, readonly string[]> = {
  onboarding: ['Boas-vindas', 'Conexões', 'Projeto', 'Time', 'Pedido', 'Pronto'],
  'novo-projeto': ['Origem', 'Time', 'Pedido'],
}

export const FLOW_PLACEHOLDER: Record<FlowId, string> = {
  onboarding: 'Pergunte à Vokia sobre a configuração',
  'novo-projeto': 'Pergunte algo sobre a configuração do projeto',
}

export const FLOW_LABEL: Record<FlowId, string> = { onboarding: 'Onboarding', 'novo-projeto': 'Novo projeto' }

export const FLOW_LAST_STEP_TITLE = 'Última etapa'
export const FLOW_READY_TEXT = 'Tudo pronto'
export const FLOW_FINISH_LABEL = 'Pronto'
