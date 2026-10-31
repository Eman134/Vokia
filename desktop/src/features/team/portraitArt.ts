export const PORTRAIT_IDS = ['iara', 'davi', 'bia', 'nina', 'leo', 'teo', 'rui'] as const
export type PortraitId = (typeof PORTRAIT_IDS)[number]
export const DEFAULT_PORTRAIT: PortraitId = 'nina'
export const DEFAULT_PORTRAIT_SIZE = 96
export const PORTRAIT_SIZES = { badge: 72, list: 40, chat: 28, stack: 24, mention: 22 } as const

export const PORTRAIT_META: Record<PortraitId, { name: string; role: string }> = {
  iara: { name: 'Iara', role: 'Tech Lead' },
  davi: { name: 'Davi', role: 'Frontend' },
  bia: { name: 'Bia', role: 'Designer' },
  nina: { name: 'Nina', role: 'Backend' },
  leo: { name: 'Léo', role: 'Integrações' },
  teo: { name: 'Téo', role: 'QA' },
  rui: { name: 'Rui', role: 'Pesquisador' },
}

export type HairStyle = 'long' | 'bob' | 'short' | 'curly' | 'bun' | 'buzz'

export interface PortraitTraits {
  bg: string
  shirt: string
  skin: string
  hair: string
  hairStyles: HairStyle[]
  beard?: boolean
  glasses?: boolean
  phones?: boolean
  cap?: string
}

export const PORTRAIT_TRAITS: Record<PortraitId, PortraitTraits> = {
  iara: { bg: '#eeebf6', shirt: '#6e5fa6', skin: '#c98e6b', hair: '#2b1d16', hairStyles: ['long'], glasses: true },
  davi: { bg: '#f1eff6', shirt: '#4b4466', skin: '#8d5a3b', hair: '#1a1210', hairStyles: ['curly'], phones: true },
  bia: { bg: '#ebe7f4', shirt: '#9387bf', skin: '#f0c8a8', hair: '#b5502f', hairStyles: ['bob'] },
  nina: { bg: '#f3f1f8', shirt: '#3d3852', skin: '#e5b48f', hair: '#3a2418', hairStyles: ['short', 'bun'] },
  leo: { bg: '#ece9f5', shirt: '#7d70ad', skin: '#d39b74', hair: '#5a3a22', hairStyles: ['short'], beard: true },
  teo: { bg: '#efedf6', shirt: '#5a5277', skin: '#6f4430', hair: '#141414', hairStyles: ['buzz'], cap: '#2b2738' },
  rui: { bg: '#e8e5f1', shirt: '#a69dc8', skin: '#f2cfb1', hair: '#9a9a9a', hairStyles: ['short'], glasses: true },
}
