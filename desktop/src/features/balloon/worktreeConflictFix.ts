// Mesmo prefixo de BRANCH_PREFIX em src-tauri/src/projects/worktree.rs.
const WORKTREE_BRANCH_PREFIX = 'vokia/'
// Texto de WorktreeError::NameConflict; o nome do item vem no grupo.
const NAME_CONFLICT_PATTERN = /Outro item já usa o nome (\S+) para branch e pasta/

export const FIX_LABEL = 'Fix'
export const FIX_SENT_TEXT = 'Pedido de correção enviado'

export function worktreeConflictName(message: string): string | undefined {
  return NAME_CONFLICT_PATTERN.exec(message)?.[1]
}

export function buildWorktreeFixPrompt(name: string, message: string): string {
  return [
    `A criação do worktree falhou: ${message}`,
    `O app cria a branch \`${WORKTREE_BRANCH_PREFIX}${name}\` e uma pasta de worktree com o nome \`${name}\`. Alguma das duas já existe.`,
    `Investigue com git (\`git worktree list\` e \`git branch --list '${WORKTREE_BRANCH_PREFIX}${name}'\`) se é resto de uma execução anterior do mesmo item ou se pertence a outro item.`,
    'Escolha a solução mais segura: reaproveitar o worktree existente, remover a branch e a pasta órfãs que não tenham commits nem alterações não commitadas, ou usar um nome com sufixo único. Nunca apague trabalho não commitado sem me perguntar.',
    'Depois resolva para que o item possa ser iniciado de novo e me diga o que fez.',
  ].join('\n\n')
}
