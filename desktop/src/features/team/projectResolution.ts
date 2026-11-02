import type { Project } from '../projects/projectsApi'

export const ASK_PROJECT_NOTICE =
  'Não sei a qual projeto este pedido se refere. Cite o nome do projeto na mensagem ou fixe um projeto na barra de ação para eu usar sempre ele.'
export const NO_FOLDER_NOTICE = 'Nenhum projeto tem uma pasta. Escolha a pasta de um projeto para enviar pedidos.'

export function citedNoFolderNotice(name: string): string {
  return `O projeto ${name} não tem pasta. Escolha a pasta dele para enviar pedidos.`
}

export type Resolution =
  | { kind: 'project'; project: Project }
  | { kind: 'ask' }
  | { kind: 'no-folder'; name?: string }

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function isCited(text: string, name: string): boolean {
  return new RegExp(`(?<!\\p{L})${escapeRegExp(name)}(?!\\p{L})`, 'iu').test(text)
}

function hasLongerCitedName(project: Project, cited: readonly Project[]): boolean {
  const name = project.name.toLowerCase()
  return cited.some((other) => {
    const otherName = other.name.toLowerCase()
    return otherName !== name && otherName.includes(name)
  })
}

function citedProjects(text: string, projects: readonly Project[]): Project[] {
  const cited = projects.filter((project) => isCited(text, project.name))
  return cited.filter((project) => !hasLongerCitedName(project, cited))
}

function hasFolder(project: Project): boolean {
  return project.path.trim() !== ''
}

function mostRecent(projects: readonly Project[]): Project | undefined {
  return projects
    .filter((project) => project.lastActivityAt !== null)
    .sort((a, b) => (b.lastActivityAt ?? 0) - (a.lastActivityAt ?? 0))[0]
}

export function resolveProject(
  text: string,
  projects: readonly Project[],
  selectedId: number | null,
): Resolution {
  const cited = citedProjects(text, projects)
  if (cited.length > 1) {
    return { kind: 'ask' }
  }
  const [citedProject] = cited
  if (citedProject !== undefined && !hasFolder(citedProject)) {
    return { kind: 'no-folder', name: citedProject.name }
  }
  const usable = projects.filter(hasFolder)
  if (usable.length === 0) {
    return { kind: 'no-folder' }
  }
  const chosen =
    citedProject ??
    usable.find((project) => project.pinned) ??
    usable.find((project) => project.id === selectedId) ??
    mostRecent(usable) ??
    (usable.length === 1 ? usable[0] : undefined)
  return chosen === undefined ? { kind: 'ask' } : { kind: 'project', project: chosen }
}
