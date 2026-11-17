import { renderAvatarArt, resolveAvatarProfile } from '../identities/avatarProfiles'
import { useAvatarProfiles } from '../identities/useAvatarProfiles'
import { useRoster } from './useRoster'

export function agentArt(who: string): string {
  return renderAvatarArt(resolveAvatarProfile(useRoster().avatarOf(who), useAvatarProfiles().profiles))
}

// Para texto que entra por v-html, onde não dá para usar o componente AgentPortrait.
export function agentIconSvg(who: string, size: number): string {
  return `<svg width="${size}" height="${size}" viewBox="0 0 64 64" aria-hidden="true">${agentArt(who)}</svg>`
}
