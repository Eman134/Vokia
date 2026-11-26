// Espelha o enum ExecutionMode de src-tauri/src/permissions/mode.rs.
export const EXECUTION_MODES = ['planejamento', 'manual', 'auto', 'bypass'] as const
export type ExecutionMode = (typeof EXECUTION_MODES)[number]
export const DEFAULT_EXECUTION_MODE: ExecutionMode = 'manual'

export interface ExecutionModeInfo {
  label: string
  description: string
  // Caminhos SVG num viewBox 16x16, traçados à mão (estilo sketch/Excalidraw).
  icon: string
  colorClass: string
  warn: boolean
}

export const EXECUTION_MODE_INFO: Record<ExecutionMode, ExecutionModeInfo> = {
  planejamento: {
    label: 'Planejamento',
    description: 'Os agentes só leem e apresentam o plano; nada é alterado.',
    icon: 'M3.6 2.9 Q8.2 2.4 12.6 2.9 Q13.1 8 12.7 13.4 Q8 13.8 3.5 13.3 Q3.1 8 3.6 2.9 M5.8 2.6 Q8 1.6 10.3 2.6 M5.4 6 Q7.9 5.7 10.6 6.1 M5.3 8.5 Q7.9 8.9 10.4 8.4 M5.5 11 Q7.1 10.8 8.7 11.1',
    colorClass: 'text-accent',
    warn: false,
  },
  manual: {
    label: 'Manual',
    description: 'Cada pedido de permissão espera a sua decisão.',
    icon: 'M5.2 9.1 C4.9 6.6 4.9 4.5 5.4 4.1 C5.9 3.7 6.5 4.4 6.6 6.9 C6.7 4.1 6.9 2.6 7.5 2.5 C8.1 2.4 8.4 3.9 8.4 6.8 C8.6 4.2 8.9 2.9 9.5 3 C10.1 3.1 10.3 4.6 10.2 7.2 C10.5 5.2 10.9 4.3 11.4 4.5 C11.9 4.7 11.9 6.6 11.7 9.1 C11.6 11.8 10.2 13.5 8 13.6 C6.1 13.6 4.8 12.6 3.7 10.8 C3.1 9.8 2.8 8.9 3.4 8.5 C4 8.1 4.7 8.8 5.2 9.5',
    colorClass: 'text-warn-text',
    warn: false,
  },
  auto: {
    label: 'Auto',
    description: 'Aprova o que é seguro dentro do projeto e só pergunta nos pedidos mais exigentes.',
    icon: 'M8 2.1 Q10.6 3.3 13 3.8 Q13.1 8.2 11.5 10.9 Q9.9 12.9 8 13.9 Q6 12.8 4.5 10.8 Q3 8.1 3.1 3.9 Q5.6 3.3 8 2.1 M5.6 8.1 Q6.7 9.4 7.3 10.1 Q9.1 7.6 10.8 5.9',
    colorClass: 'text-ok',
    warn: false,
  },
  bypass: {
    label: 'Bypass total',
    description: 'Aprova tudo sozinho, sem pedir confirmação. Use só em projetos de confiança.',
    icon: 'M9.3 1.9 Q6.7 5.3 4.1 8.6 Q6.1 8.4 7.8 8.6 Q7.1 11.4 6.5 14.1 Q9.4 10.5 12.1 7.1 Q10.1 7.3 8.5 7.1 Q9 4.5 9.3 1.9 M12.2 4.3 Q13.1 3.9 13.6 3.3 M12.5 7.4 Q13.5 7.5 14.1 7.8',
    colorClass: 'text-danger',
    warn: true,
  },
}
