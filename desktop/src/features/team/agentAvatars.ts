export const AGENT_AVATARS: Record<string, string> = {
  iara: 'quarzo',
  davi: 'lume',
  bia: 'ambar',
  nina: 'vesper',
  leo: 'onix',
  teo: 'selene',
  rui: 'ambar-ouro',
}

// Sem entrada no mapa, o id do agente vira o id do perfil; resolveAvatarProfile cai num perfil determinístico.
export function avatarOf(agentId: string): string {
  const key = agentId.trim().toLowerCase()
  return AGENT_AVATARS[key] ?? key
}
