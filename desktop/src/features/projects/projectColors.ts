// As chaves casam com os tokens --color-project-* de styles/tokens.css e com PROJECT_COLORS de projects/manage.rs.
export const PROJECT_COLORS = {
  rose: { label: 'Rosa' },
  peach: { label: 'Pêssego' },
  yellow: { label: 'Amarelo' },
  green: { label: 'Verde' },
  teal: { label: 'Turquesa' },
  blue: { label: 'Azul' },
  purple: { label: 'Roxo' },
  pink: { label: 'Magenta' },
} as const

export type ProjectColorKey = keyof typeof PROJECT_COLORS

export const PROJECT_COLOR_KEYS = Object.keys(PROJECT_COLORS) as ProjectColorKey[]

function isProjectColorKey(key: string): key is ProjectColorKey {
  return key in PROJECT_COLORS
}

// Sem escolha gravada, a cor sai do id: o mesmo projeto tem sempre a mesma cor.
export function projectColorOf(project: { id: number; color: string | null }): ProjectColorKey {
  if (project.color !== null && isProjectColorKey(project.color)) return project.color
  return PROJECT_COLOR_KEYS[project.id % PROJECT_COLOR_KEYS.length]
}
