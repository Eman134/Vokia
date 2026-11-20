export interface ErrorSummary {
  title: string
  detail: string
}

// A primeira linha com texto vira o título; o resto vai como detalhe. Sem mensagem, nada é inventado.
export function errorSummary(message: string): ErrorSummary {
  const lines = message
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '')
  return { title: lines[0] ?? '', detail: lines.slice(1).join('\n') }
}
