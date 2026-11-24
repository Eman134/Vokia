import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'

const TILT_LIMIT_DEGREES = 0.6
const TILT_STEPS = 13

// Folha solta: o papel pousa um pouco torto, e o mesmo id sempre cai com a mesma inclinação.
export const CARD_CLASSES = `relative flex min-w-0 flex-col gap-3 rounded-sm border border-line bg-surface p-4 text-left shadow-paper transition-transform hover:-translate-y-px motion-reduce:transition-none ${INK_FOCUS_CLASSES}`

export function sheetTilt(id: string): { transform: string } {
  const hash = [...id].reduce((sum, char) => sum + char.charCodeAt(0), 0)
  const degrees = ((hash % TILT_STEPS) / (TILT_STEPS - 1)) * 2 * TILT_LIMIT_DEGREES - TILT_LIMIT_DEGREES
  return { transform: `rotate(${degrees.toFixed(2)}deg)` }
}
export const WRAP_CLASSES = 'wrap-break-word'
export const SUMMARY_LINES_CLASSES = 'line-clamp-3'
// Preso ao fim do card: na mesma linha da grade, barras e datas ficam na mesma altura.
export const CARD_FOOTER_CLASSES = 'mt-auto flex flex-col gap-1.5 text-meta text-muted'
