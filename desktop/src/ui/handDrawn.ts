// Raios assimétricos dão o contorno irregular de traço à mão; o `!` vence o raio base do elemento.
export const HAND_DRAWN_RADIUS = 'rounded-[7px_11px_8px_12px/11px_7px_12px_8px]!'
// O destaque é a tinta, não a cor de acento.
export const HAND_OUTLINE_CLASSES = `border-[1.5px] border-ink ${HAND_DRAWN_RADIUS}`
export const HAND_SELECTED_CLASSES = `${HAND_OUTLINE_CLASSES} bg-surface-2`
// Base do sublinhado à mão (UiHandLine) do item ativo; cada uso posiciona e dá largura explícita.
export const HAND_MARK_CLASSES = 'pointer-events-none absolute h-1.5 text-ink'
export const INK_FOCUS_CLASSES =
  'focus:outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-ink'
// Post-it: o amarelo é pastel nos dois temas, então tinta e muted são refeitos com mark-ink para valer também nos botões internos.
// bg e surface viram o próprio amarelo: o mascote pinta olhos e boca com `surface` sobre um corpo `ink`, que sem isso ficariam escuros sobre escuro.
export const STICKY_INK_VARS =
  '[--color-ink:var(--color-mark-ink)] [--color-muted:color-mix(in_srgb,var(--color-mark-ink)_70%,transparent)] [--color-ok:var(--color-mark-ink)] [--color-bg:var(--color-mark-yellow)] [--color-surface:var(--color-mark-yellow)]'
// Pedido do usuário: marca-texto azul suave e contorno irregular, sem a bolha arredondada de chat.
export const USER_BUBBLE_CLASSES = 'rounded-md bg-surface-2'

// Mini traço no hover (~30% da largura) que se expande ao selecionar; o host precisa da classe `group` e de `relative`.
const HAND_REVEAL_BASE = 'pointer-events-none origin-left transition-transform duration-180 ease-out motion-reduce:transition-none'
const HAND_REVEAL_IDLE = 'scale-x-0 group-hover:scale-x-30 group-focus-visible:scale-x-30'
const HAND_REVEAL_ON = 'scale-x-100'

export function handReveal(active: boolean): string {
  return `${HAND_REVEAL_BASE} ${active ? HAND_REVEAL_ON : HAND_REVEAL_IDLE}`
}
