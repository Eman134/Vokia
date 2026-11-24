// Centraliza na janela, mas a borda esquerda nunca entra na coluna fixa do trilho (--rail-w vem de App.vue).
// O invólucro deixa passar o ponteiro: só a pílula captura, para o resto da janela continuar clicável.
export const BAR_MOTION_CLASSES = 'duration-180 ease-out motion-reduce:transition-none'
// Em janelas baixas a barra desce um pouco para devolver altura útil ao conteúdo.
const BAR_BOTTOM_CLASSES = 'bottom-6 [@media(max-height:720px)]:bottom-4'
export const BAR_WRAP_CLASSES = `pointer-events-none fixed ${BAR_BOTTOM_CLASSES} left-[max(--spacing(6),calc(50%-var(--container-bar)/2),var(--rail-w,0px))] right-[max(--spacing(6),calc(50%-var(--container-bar)/2))] flex justify-center transition-[left] ${BAR_MOTION_CLASSES}`
// Largura e altura da pílula são animadas pelo `layout` do Motion (FLIP com mola), não por transição CSS.
export const PILL_CLASSES =
  'pointer-events-auto relative flex h-12 items-center gap-3 rounded-full border border-line bg-surface pl-2.5 pr-1.5 shadow-bar'
// O conteúdo some aos poucos antes da barra, em vez de aparecer cortado atrás dela; não captura o ponteiro.
export const BAR_FADE_CLASSES = `pointer-events-none fixed bottom-0 left-[var(--rail-w,0px)] right-0 h-24 bg-linear-to-t from-bg to-transparent transition-[left] ${BAR_MOTION_CLASSES}`
export const BAR_SPRING = { type: 'spring', stiffness: 850, damping: 58 } as const
// Rótulo do recolhimento do texto de status (overflow-hidden), enquanto a pílula muda de tamanho.
export const BAR_COLLAPSE_TOTAL_MS = 400
