export const LOGO_VIEWBOX = 64

export const LOGO_SIZES = [64, 48, 32, 24, 20, 16] as const
export type LogoSize = (typeof LOGO_SIZES)[number]
export type LogoVariant = 'cheio' | 'contorno' | 'gota' | 'cor'

export const LOGO_V_PATH = 'M6 16 H20 L32 44 L44 16 H58 L38 60 H26 Z'
export const LOGO_FLAME_PATH = 'M32 40 C 24 34, 23 25, 28 18 C 31 14, 31 9, 29 3 C 38 7, 43 15, 41 24 C 40 29, 37 33, 32 40 Z'
export const LOGO_FLAME_CORE_PATH = 'M32 34 C 29 31, 28 27, 30 23 C 31 21, 32 19, 32 16 C 35 19, 36 23, 35 27 C 34 30, 33 32, 32 34 Z'
export const LOGO_FLAME_CUT_PATH = 'M32 44 L36 34 C 34 37, 33 39, 32 40 C 31 39, 30 37, 28 34 Z'
export const LOGO_DROP_PATH = 'M32 3 C 36 12, 52 24, 52 40 a20 20 0 0 1 -40 0 C 12 24, 28 12, 32 3 Z'
export const LOGO_DROP_CUT_PATH = 'M20 30 H28 L32 41 L36 30 H44 L35 52 H29 Z'

export const BRAND_GRADIENT = {
  v: { from: '#f08a3c', to: '#e0524a' },
  flame: { from: '#f08a3c', to: '#f6b24a' },
  core: '#ffe08a',
} as const
