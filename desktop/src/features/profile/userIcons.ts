export interface UserIcon {
  id: string
  label: string
  background: string
  // Desenho branco em viewBox 64, sobre o disco colorido.
  glyph: string
}

export interface UserAvatarChoice {
  icone: string
}

const WHITE = '#fff'
const stroke = (paths: string): string =>
  `<g fill="none" stroke="${WHITE}" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round">${paths}</g>`
const fill = (shapes: string): string => `<g fill="${WHITE}">${shapes}</g>`

export const USER_ICONS: readonly UserIcon[] = [
  {
    id: 'sol',
    label: 'Sol',
    background: '#e8a22f',
    glyph:
      fill('<circle cx="32" cy="32" r="8"></circle>') +
      stroke(
        '<path d="M32 15v5M32 44v5M15 32h5M44 32h5M20 20l3.5 3.5M40.5 40.5L44 44M44 20l-3.5 3.5M23.5 40.5L20 44"></path>',
      ),
  },
  {
    id: 'lua',
    label: 'Lua',
    background: '#4a5bd4',
    glyph: fill('<path d="M38 15A17 17 0 1 0 49 39 14 14 0 0 1 38 15Z"></path>'),
  },
  {
    id: 'estrela',
    label: 'Estrela',
    background: '#d9568a',
    glyph: fill(
      '<polygon points="32,14 36.5,26 49.5,26.8 39.5,35 42.8,47.5 32,40.5 21.2,47.5 24.5,35 14.5,26.8 27.5,26"></polygon>',
    ),
  },
  {
    id: 'raio',
    label: 'Raio',
    background: '#7a56d9',
    glyph: fill('<polygon points="36,13 21,35 30,35 27,51 43,28 34,28"></polygon>'),
  },
  {
    id: 'folha',
    label: 'Folha',
    background: '#2f9e6d',
    glyph:
      fill('<path d="M18 46C18 29 29 17 47 17 47 35 38 46 18 46Z"></path>') +
      '<path d="M18 46L33 31" fill="none" stroke="#2f9e6d" stroke-width="2.5" stroke-linecap="round"></path>',
  },
  {
    id: 'onda',
    label: 'Onda',
    background: '#2a9bb8',
    glyph: stroke(
      '<path d="M14 27q4.5-7 9 0t9 0 9 0 9 0"></path><path d="M14 39q4.5-7 9 0t9 0 9 0 9 0"></path>',
    ),
  },
  {
    id: 'montanha',
    label: 'Montanha',
    background: '#5a6478',
    glyph: fill('<path d="M13 47L27 23 35 36 40 29 51 47Z"></path>'),
  },
  {
    id: 'coracao',
    label: 'Coração',
    background: '#e0524a',
    glyph: fill('<path d="M32 48C13 36 17 20 27 21.5 30.5 22 32 25 32 25S33.5 22 37 21.5C47 20 51 36 32 48Z"></path>'),
  },
]

export function findUserIcon(id: string): UserIcon | undefined {
  return USER_ICONS.find((icon) => icon.id === id)
}
