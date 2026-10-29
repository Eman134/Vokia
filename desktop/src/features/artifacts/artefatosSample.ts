import type { PortraitId } from '../team/portraitArt'

export const ARTIFACT_KINDS = ['whiteboard', 'mermaid', 'plantuml', 'documento', 'casos', 'anotacoes', 'adr'] as const
export type ArtifactKind = (typeof ARTIFACT_KINDS)[number]

export const ALL_FILTER = 'todos'
export type ArtifactFilter = typeof ALL_FILTER | ArtifactKind

interface ArtifactKindInfo {
  label: string
  iconPath: string
}

export const ARTIFACT_KIND_INFO: Record<ArtifactKind, ArtifactKindInfo> = {
  whiteboard: {
    label: 'Whiteboard',
    iconPath: 'M2 2.5h5v4H2zM9.5 9.5a2.5 2.5 0 105 0 2.5 2.5 0 00-5 0zM2 13.5l3-4 3 4zM7 4.5h3.5v3',
  },
  mermaid: {
    label: 'Mermaid',
    iconPath: 'M3 2.5h4v3H3zM9 10.5h4v3H9zM5 5.5v2.5h6v2.5',
  },
  plantuml: {
    label: 'PlantUML',
    iconPath: 'M2 2.5h5v4H2zM9 9.5h5v4H9zM2 4.5h5M9 11.5h5M4.5 6.5v5H9',
  },
  documento: {
    label: 'Documento',
    iconPath: 'M4 1.5h5.5l3 3v10H4zM9.5 1.5v3h3M6 8h4.5M6 10.5h4.5',
  },
  casos: {
    label: 'Casos de uso',
    iconPath:
      'M4.5 3.5a1.5 1.5 0 100-.1M4.5 5.2v4M2.5 6.8h4M4.5 9.2L3 13M4.5 9.2L6 13M9 7.5c0-1.4 1.3-2.5 3-2.5s3 1.1 3 2.5-1.3 2.5-3 2.5-3-1.1-3-2.5z',
  },
  anotacoes: {
    label: 'Anotações',
    iconPath: 'M3 2.5h10v8l-3 3H3zM10 13.5v-3h3M5.5 6h5M5.5 8.5h3',
  },
  adr: {
    label: 'Decisão (ADR)',
    iconPath: 'M8 2.5v11M4.5 13.5h7M3 5h10M3 5L1.5 9h3zM13 5l-1.5 4h3z',
  },
}

interface ArtifactFilterOption {
  value: ArtifactFilter
  label: string
}

export const ARTIFACT_FILTERS: readonly ArtifactFilterOption[] = [
  { value: ALL_FILTER, label: 'Todos' },
  ...ARTIFACT_KINDS.map((kind) => ({ value: kind, label: ARTIFACT_KIND_INFO[kind].label })),
]

export interface ArtifactSample {
  kind: ArtifactKind
  title: string
  author: string
  portrait?: PortraitId
  time: string
}

export const ARTIFACT_SAMPLES: readonly ArtifactSample[] = [
  { kind: 'whiteboard', title: 'Arquitetura do checkout', author: 'Iara', portrait: 'iara', time: 'agora' },
  { kind: 'mermaid', title: 'Fluxo do pagamento Pix', author: 'Nina', portrait: 'nina', time: '4 min' },
  { kind: 'plantuml', title: 'Modelo de domínio de pagamentos', author: 'Iara', portrait: 'iara', time: '1 h' },
  { kind: 'documento', title: 'Spec · Pagamento via Pix', author: 'Téo', portrait: 'teo', time: '20 min' },
  { kind: 'casos', title: 'Casos de uso do checkout', author: 'Rui', portrait: 'rui', time: 'ontem' },
  { kind: 'anotacoes', title: 'Anotações do checkout', author: 'Rui', portrait: 'rui', time: '2 h' },
  { kind: 'adr', title: 'ADR-007 · Efí como PSP de Pix', author: 'você', time: 'terça' },
  { kind: 'mermaid', title: 'Estados do pedido', author: 'Davi', portrait: 'davi', time: 'ontem' },
  { kind: 'documento', title: 'Runbook · webhook fora do ar', author: 'Nina', portrait: 'nina', time: '3 dias' },
]

export function artifactsOf(filter: ArtifactFilter): readonly ArtifactSample[] {
  return filter === ALL_FILTER ? ARTIFACT_SAMPLES : ARTIFACT_SAMPLES.filter((artifact) => artifact.kind === filter)
}

export const NEW_ARTIFACT_LABEL = '+ Novo artefato'
export const PAGE_SUBTITLE =
  'Quadros, diagramas, documentos e decisões do projeto. Você e os agentes criam e editam, e tudo fica salvo no repositório.'
