import type { IdentityPaleta } from './identityFamilies'

export type AvatarShapeId = 'gota' | 'cristal-hexagono' | 'cristal-losango' | 'cristal-prisma'
export type AvatarExpression = 'feliz' | 'curioso' | 'focado' | 'sonolento'

export interface AvatarColor {
  id: string
  label: string
  base: string
  shade: string
  paleta: IdentityPaleta
}

export interface AvatarShape {
  id: AvatarShapeId
  label: string
  body: (color: AvatarColor) => string
  faces: Partial<Record<AvatarExpression, string>>
}

export interface AvatarProfile {
  id: string
  nome: string
  forma: AvatarShapeId
  cor: string
  expressao: AvatarExpression
}

export const AVATAR_COLORS: AvatarColor[] = [
  { id: 'azul', label: 'Azul', base: '#4f7cff', shade: '#2f56d6', paleta: 'fria' },
  { id: 'laranja', label: 'Laranja', base: '#f08a3c', shade: '#c8611c', paleta: 'quente' },
  { id: 'amarelo', label: 'Amarelo', base: '#e8b931', shade: '#b88c12', paleta: 'quente' },
  { id: 'vermelho', label: 'Vermelho', base: '#e0524a', shade: '#b0332c', paleta: 'quente' },
  { id: 'rosa', label: 'Rosa', base: '#e0609a', shade: '#b83d74', paleta: 'quente' },
  { id: 'roxo', label: 'Roxo', base: '#8b6cf0', shade: '#6345cc', paleta: 'fria' },
  { id: 'verde-agua', label: 'Verde-água', base: '#2bb3a8', shade: '#1a857c', paleta: 'fria' },
  { id: 'onix', label: 'Ônix', base: '#2b2f4a', shade: '#1a1d33', paleta: 'neutra' },
]

export const AVATAR_SHAPES: AvatarShape[] = [
  {
    id: 'gota',
    label: 'Gota',
    body: (color) =>
      `<ellipse cx="32" cy="58" rx="14" ry="2.5" fill="#1c1c21" opacity=".12"></ellipse><path d="M32 5 C 26 16, 12 28, 12 39 a20 19 0 0 0 40 0 C 52 28, 38 16, 32 5 Z" fill="${color.base}"></path><path d="M32 5 C 38 16, 52 28, 52 39 a20 19 0 0 1 -20 19 C 44 52, 47 40, 32 5 Z" fill="${color.shade}" opacity=".45"></path><ellipse cx="22" cy="30" rx="3" ry="5" fill="#fff" opacity=".55" transform="rotate(25 22 30)"></ellipse>`,
    faces: {
      feliz: `<circle cx="25" cy="40" r="4.2" fill="#fff"></circle><circle cx="25" cy="40" r="2.1" fill="#1c1c21"></circle><circle cx="39" cy="40" r="4.2" fill="#fff"></circle><circle cx="39" cy="40" r="2.1" fill="#1c1c21"></circle><path d="M27 48 Q32 53 37 48" fill="none" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path>`,
      curioso: `<circle cx="25" cy="40" r="4.8" fill="#fff"></circle><circle cx="26.4" cy="39" r="2.4" fill="#1c1c21"></circle><circle cx="39" cy="40" r="4.8" fill="#fff"></circle><circle cx="40.4" cy="39" r="2.4" fill="#1c1c21"></circle><circle cx="32" cy="49" r="2.2" fill="#1c1c21"></circle>`,
      focado: `<path d="M21 33 L28 34.5M43 33 L36 34.5" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path><circle cx="25" cy="40" r="4.2" fill="#fff"></circle><circle cx="25" cy="41.2" r="2.1" fill="#1c1c21"></circle><circle cx="39" cy="40" r="4.2" fill="#fff"></circle><circle cx="39" cy="41.2" r="2.1" fill="#1c1c21"></circle><path d="M28 49 H36" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path>`,
      sonolento: `<path d="M20.8 40 Q25 43.78 29.2 40" fill="none" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path><path d="M34.8 40 Q39 43.78 43.2 40" fill="none" stroke="#1c1c21" stroke-width="2" stroke-linecap="round"></path><path d="M29 49 Q32 51 35 49" fill="none" stroke="#1c1c21" stroke-width="1.8" stroke-linecap="round"></path>`,
    },
  },
  {
    id: 'cristal-hexagono',
    label: 'Cristal hexágono',
    body: (color) =>
      `<polygon points="32,4 54,17 54,45 32,58 10,45 10,17" fill="${color.base}"></polygon><polygon points="32,4 54,17 32,30 10,17" fill="#fff" opacity=".35"></polygon><polygon points="32,30 54,17 54,45 32,58" fill="${color.shade}" opacity=".55"></polygon><path d="M32 30 V58M10 17 L32 30 L54 17" stroke="#fff" stroke-opacity=".5" stroke-width="1.2" fill="none"></path>`,
    faces: {
      feliz: `<circle cx="25" cy="36" r="3.2" fill="#fff"></circle><circle cx="25" cy="36" r="1.6" fill="#1c1c21"></circle><circle cx="39" cy="36" r="3.2" fill="#fff"></circle><circle cx="39" cy="36" r="1.6" fill="#1c1c21"></circle>`,
    },
  },
  {
    id: 'cristal-losango',
    label: 'Cristal losango',
    body: (color) =>
      `<polygon points="32,3 56,30 32,61 8,30" fill="${color.base}"></polygon><polygon points="32,3 56,30 32,30" fill="${color.shade}" opacity=".6"></polygon><polygon points="8,30 32,30 32,61" fill="#fff" opacity=".25"></polygon><path d="M8 30 H56M32 3 V61" stroke="#fff" stroke-opacity=".45" stroke-width="1.2"></path>`,
    faces: {
      feliz: `<circle cx="26" cy="38" r="3" fill="#fff"></circle><circle cx="26" cy="38" r="1.5" fill="#1c1c21"></circle><circle cx="38" cy="38" r="3" fill="#fff"></circle><circle cx="38" cy="38" r="1.5" fill="#1c1c21"></circle>`,
    },
  },
  {
    id: 'cristal-prisma',
    label: 'Cristal prisma',
    body: (color) =>
      `<polygon points="32,2 46,22 40,62 24,62 18,22" fill="${color.base}"></polygon><polygon points="32,2 46,22 32,26 18,22" fill="#fff" opacity=".4"></polygon><polygon points="32,26 46,22 40,62 32,62" fill="${color.shade}" opacity=".55"></polygon><path d="M32 26 V62" stroke="#fff" stroke-opacity=".5" stroke-width="1.2"></path>`,
    faces: {
      feliz: `<circle cx="28" cy="36" r="2.8" fill="#fff"></circle><circle cx="28" cy="36" r="1.4" fill="#1c1c21"></circle><circle cx="37" cy="36" r="2.8" fill="#fff"></circle><circle cx="37" cy="36" r="1.4" fill="#1c1c21"></circle>`,
    },
  },
]

// A ordem define o fallback por hash: reordenar muda o avatar de ids sem perfil.
export const AVATAR_PROFILES: AvatarProfile[] = [
  { id: 'lume', nome: 'Lume', forma: 'gota', cor: 'azul', expressao: 'feliz' },
  { id: 'ambar', nome: 'Âmbar', forma: 'gota', cor: 'laranja', expressao: 'curioso' },
  { id: 'vesper', nome: 'Vésper', forma: 'gota', cor: 'roxo', expressao: 'sonolento' },
  { id: 'quarzo', nome: 'Quarzo', forma: 'cristal-hexagono', cor: 'verde-agua', expressao: 'feliz' },
  { id: 'onix', nome: 'Ônix', forma: 'cristal-losango', cor: 'roxo', expressao: 'feliz' },
  { id: 'selene', nome: 'Selene', forma: 'cristal-prisma', cor: 'rosa', expressao: 'feliz' },
  { id: 'ambar-ouro', nome: 'Âmbar ouro', forma: 'gota', cor: 'amarelo', expressao: 'curioso' },
  { id: 'ambar-rubi', nome: 'Âmbar rubi', forma: 'gota', cor: 'vermelho', expressao: 'curioso' },
  { id: 'ambar-rosa', nome: 'Âmbar rosa', forma: 'gota', cor: 'rosa', expressao: 'curioso' },
  { id: 'vesper-agua', nome: 'Vésper água', forma: 'gota', cor: 'verde-agua', expressao: 'sonolento' },
  { id: 'onix-negro', nome: 'Ônix negro', forma: 'cristal-losango', cor: 'onix', expressao: 'feliz' },
  { id: 'selene-ouro', nome: 'Selene ouro', forma: 'cristal-prisma', cor: 'amarelo', expressao: 'feliz' },
]

const warnedIds = new Set<string>()

function hashAvatarId(key: string): number {
  let sum = 0
  for (let index = 0; index < key.length; index++) sum += key.charCodeAt(index)
  return sum
}

function findShape(id: AvatarShapeId): AvatarShape | undefined {
  return AVATAR_SHAPES.find((shape) => shape.id === id)
}

function findColor(id: string): AvatarColor | undefined {
  return AVATAR_COLORS.find((color) => color.id === id)
}

function isRenderable(profile: AvatarProfile): boolean {
  return findColor(profile.cor) !== undefined && findShape(profile.forma)?.faces[profile.expressao] !== undefined
}

export function resolveAvatarProfile(id: string): AvatarProfile {
  const key = id.trim().toLowerCase()
  const profile = AVATAR_PROFILES.find((candidate) => candidate.id === key)
  if (profile && isRenderable(profile)) return profile
  if (!warnedIds.has(key)) {
    warnedIds.add(key)
    console.warn(`Perfil de avatar inválido ou ausente: ${key}`)
  }
  return AVATAR_PROFILES[hashAvatarId(key) % AVATAR_PROFILES.length]
}

export function renderAvatarArt(profile: AvatarProfile): string {
  const shape = findShape(profile.forma)
  const color = findColor(profile.cor)
  const face = shape?.faces[profile.expressao]
  if (!shape || !color || face === undefined) throw new Error(`Perfil de avatar inconsistente: ${profile.id}`)
  return shape.body(color) + face
}
