import { ARTIFACT_KIND_INFO } from '../artifacts/artefatosSample'

interface ProjectIconInfo {
  label: string
  path: string
}

// As chaves são gravadas em projects.symbol e validadas pela allowlist PROJECT_ICONS de projects.rs.
export const PROJECT_ICONS = {
  folder: { label: 'Pasta', path: 'M2 4h4.5L8 5.5h6V13H2z' },
  target: { label: 'Alvo', path: 'M8 2.5a5.5 5.5 0 110 11 5.5 5.5 0 010-11zM8 6a2 2 0 110 4 2 2 0 010-4z' },
  triangle: { label: 'Triângulo', path: 'M8 2.5l5.5 10h-11z' },
  square: { label: 'Quadrado', path: 'M3.5 3.5h9v9h-9z' },
  diamond: { label: 'Losango', path: 'M8 2l6 6-6 6-6-6z' },
  document: { label: 'Documento', path: ARTIFACT_KIND_INFO.documento.iconPath },
  diagram: { label: 'Diagrama', path: ARTIFACT_KIND_INFO.mermaid.iconPath },
  whiteboard: { label: 'Whiteboard', path: ARTIFACT_KIND_INFO.whiteboard.iconPath },
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
