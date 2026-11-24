// Espelha o enum ExecutionMode de src-tauri/src/permissions/mode.rs.
export const EXECUTION_MODES = ['planejamento', 'manual', 'auto', 'bypass'] as const
export type ExecutionMode = (typeof EXECUTION_MODES)[number]
export const DEFAULT_EXECUTION_MODE: ExecutionMode = 'manual'

export interface ExecutionModeInfo {
  label: string
  description: string
  // Caminhos SVG num viewBox 16x16, traçados com stroke.
  icon: string
  warn: boolean
}

export const EXECUTION_MODE_INFO: Record<ExecutionMode, ExecutionModeInfo> = {
  planejamento: {
    label: 'Planejamento',
    description: 'Os agentes só leem e apresentam o plano; nada é alterado.',
    icon: 'M3 2.5h10v11H3z M5.5 5.5h5 M5.5 8h5 M5.5 10.5h3',
    warn: false,
  },
  manual: {
    label: 'Manual',
    description: 'Cada pedido de permissão espera a sua decisão.',
    icon: 'M5.5 8V3.5a1 1 0 0 1 2 0V7 M7.5 7V2.5a1 1 0 0 1 2 0V7 M9.5 7V3.5a1 1 0 0 1 2 0V9c0 2.8-1.7 4.5-4 4.5-1.8 0-2.7-.8-3.6-2.2L2.6 8.6a1 1 0 0 1 1.6-1.1l1.3 1.5',
    warn: false,
  },
  auto: {
    label: 'Auto',
    description: 'Aprova o que é seguro dentro do projeto e só pergunta nos pedidos mais exigentes.',
    icon: 'M8 2l5 2v4c0 3-2.2 5-5 6-2.8-1-5-3-5-6V4z M5.8 8.2l1.6 1.6 3-3.2',
    warn: false,
  },
  bypass: {
    label: 'Bypass total',
    description: 'Aprova tudo sozinho, sem pedir confirmação. Use só em projetos de confiança.',
    icon: 'M9 1.5L3.5 9H8l-1 5.5L12.5 7H8z',
    warn: true,
  },
}
