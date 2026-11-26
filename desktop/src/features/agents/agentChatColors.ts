export interface AgentChatColorOption {
  id: string
  label: string
  swatchClass: string
  borderClass: string
}

export const AGENT_CHAT_COLORS: readonly AgentChatColorOption[] = [
  { id: 'accent', label: 'Caneta azul', swatchClass: 'bg-accent', borderClass: 'border-accent' },
  { id: 'project-rose', label: 'Rosa', swatchClass: 'bg-project-rose', borderClass: 'border-project-rose' },
  { id: 'project-peach', label: 'Pêssego', swatchClass: 'bg-project-peach', borderClass: 'border-project-peach' },
  { id: 'project-yellow', label: 'Amarelo', swatchClass: 'bg-project-yellow', borderClass: 'border-project-yellow' },
  { id: 'project-green', label: 'Verde', swatchClass: 'bg-project-green', borderClass: 'border-project-green' },
  { id: 'project-teal', label: 'Turquesa', swatchClass: 'bg-project-teal', borderClass: 'border-project-teal' },
  { id: 'project-blue', label: 'Azul', swatchClass: 'bg-project-blue', borderClass: 'border-project-blue' },
  { id: 'project-purple', label: 'Roxo', swatchClass: 'bg-project-purple', borderClass: 'border-project-purple' },
  { id: 'project-pink', label: 'Magenta', swatchClass: 'bg-project-pink', borderClass: 'border-project-pink' },
]

export function findChatColor(id: string | null | undefined): AgentChatColorOption | null {
  if (!id) return null
  return AGENT_CHAT_COLORS.find((option) => option.id === id) ?? null
}
