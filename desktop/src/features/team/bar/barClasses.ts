// Centraliza na janela, mas a borda esquerda nunca entra na coluna fixa do trilho (--rail-w vem de App.vue).
// O invólucro deixa passar o ponteiro: só a pílula captura, para o resto da janela continuar clicável.
export const BAR_WRAP_CLASSES =
  'pointer-events-none fixed bottom-6 left-[max(--spacing(6),calc(50%-var(--container-bar)/2),var(--rail-w,0px))] right-[max(--spacing(6),calc(50%-var(--container-bar)/2))] flex justify-center'
export const BAR_MOTION_CLASSES = 'duration-180 ease-out motion-reduce:transition-none'
// `relative` ancora o menu de @ (MentionMenu é absolute) à pílula, não ao invólucro.
// interpolate-size deixa o width animar entre w-fit e w-full; sem suporte a largura só troca.
export const PILL_CLASSES = `pointer-events-auto relative flex h-12 items-center gap-3 rounded-full border border-line bg-surface pl-2.5 pr-1.5 shadow-float transition-[width] [interpolate-size:allow-keywords] ${BAR_MOTION_CLASSES}`
