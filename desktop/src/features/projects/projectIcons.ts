interface ProjectIconInfo {
  label: string
  path: string
}

// As chaves são gravadas em projects.symbol e validadas pela allowlist PROJECT_ICONS de projects.rs.
export const PROJECT_ICONS = {
  folder: { label: 'Pasta', path: 'M2 4h4.5L8 5.5h6V13H2z' },
  circle: { label: 'Círculo', path: 'M8 2.5a5.5 5.5 0 110 11 5.5 5.5 0 010-11z' },
  target: { label: 'Alvo', path: 'M8 2.5a5.5 5.5 0 110 11 5.5 5.5 0 010-11zM8 6a2 2 0 110 4 2 2 0 010-4z' },
  triangle: { label: 'Triângulo', path: 'M8 2.5l5.5 10h-11z' },
  square: { label: 'Quadrado', path: 'M3.5 3.5h9v9h-9z' },
  diamond: { label: 'Losango', path: 'M8 2l6 6-6 6-6-6z' },
  document: { label: 'Documento', path: 'M4 1.5h5.5l3 3v10H4zM9.5 1.5v3h3M6 8h4.5M6 10.5h4.5' },
  diagram: { label: 'Diagrama', path: 'M3 2.5h4v3H3zM9 10.5h4v3H9zM5 5.5v2.5h6v2.5' },
  whiteboard: { label: 'Whiteboard', path: 'M2 2.5h5v4H2zM9.5 9.5a2.5 2.5 0 105 0 2.5 2.5 0 00-5 0zM2 13.5l3-4 3 4zM7 4.5h3.5v3' },
  star: { label: 'Estrela', path: 'M8 2.3L9.5 6.3L13.7 6.45L10.4 9.1L11.5 13.15L8 10.8L4.5 13.15L5.6 9.1L2.3 6.45L6.5 6.3Z' },
  heart: { label: 'Coração', path: 'M8 13.5C3 10 2.5 7.5 3.5 5.5C4.5 3.5 7.2 3.5 8 5.5C8.8 3.5 11.5 3.5 12.5 5.5C13.5 7.5 13 10 8 13.5Z' },
  bolt: { label: 'Raio', path: 'M9 2.5L4.5 9H8L7 13.5L11.5 7H8Z' },
  flag: { label: 'Bandeira', path: 'M3.5 14V2.5M3.5 3H12L10.5 5.5L12 8H3.5' },
  bookmark: { label: 'Marcador', path: 'M4.5 2.5H11.5V13.5L8 10.5L4.5 13.5Z' },
  hexagon: { label: 'Hexágono', path: 'M8 2.5L13 5.25V10.75L8 13.5L3 10.75V5.25Z' },
  cross: { label: 'Cruz', path: 'M6 2.5H10V6H13.5V10H10V13.5H6V10H2.5V6H6Z' },
  moon: { label: 'Lua', path: 'M11 2.5A5.5 5.5 0 1 0 13.5 10.5A4.5 4.5 0 0 1 11 2.5Z' },
  cloud: { label: 'Nuvem', path: 'M4.5 12.5H12A2.75 2.75 0 0 0 12 7A4 4 0 0 0 4.7 6.2A2.8 2.8 0 0 0 4.5 12.5Z' },
  leaf: { label: 'Folha', path: 'M3 13C3 7 6.5 3 13 3C13 9.5 9.5 13 3 13ZM3 13L9 7' },
  bars: { label: 'Gráfico', path: 'M3 13.5H13.5M4.5 11V8M8 11V4.5M11.5 11V6.5' },
  lock: { label: 'Cadeado', path: 'M4.5 7H11.5V13.5H4.5ZM6 7V5A2 2 0 0 1 10 5V7' },
  bell: { label: 'Sino', path: 'M4 11.5V7A4 4 0 0 1 12 7V11.5L13 12.5H3ZM6.5 14H9.5' },
  mountain: { label: 'Montanha', path: 'M2 13L6 6L8.5 9.5L10 7.5L14 13Z' },
  sun: { label: 'Sol', path: 'M8 5.5A2.5 2.5 0 1 0 8 10.5A2.5 2.5 0 1 0 8 5.5ZM8 1.5V3M8 13V14.5M1.5 8H3M13 8H14.5M3.6 3.6L4.7 4.7M11.3 11.3L12.4 12.4M12.4 3.6L11.3 4.7M4.7 11.3L3.6 12.4' },
} as const satisfies Record<string, ProjectIconInfo>

export type ProjectIconKey = keyof typeof PROJECT_ICONS

export const PROJECT_ICON_KEYS = Object.keys(PROJECT_ICONS) as ProjectIconKey[]
export const DEFAULT_PROJECT_ICON: ProjectIconKey = PROJECT_ICON_KEYS[0]

function isProjectIconKey(key: string): key is ProjectIconKey {
  return key in PROJECT_ICONS
}

// Projetos gravados antes do catálogo podem ter letras em symbol: caem no ícone padrão.
export function projectIconOf(key: string): ProjectIconInfo {
  return PROJECT_ICONS[isProjectIconKey(key) ? key : DEFAULT_PROJECT_ICON]
}
