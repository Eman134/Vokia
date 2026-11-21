import { invoke } from '@tauri-apps/api/core'
import type { BacklogItem } from '../backlog/backlogTypes'

const LIST_ARTIFACT_KINDS_COMMAND = 'list_artifact_kinds'
const LIST_ARTIFACTS_COMMAND = 'list_artifacts'
const GET_ARTIFACT_COMMAND = 'get_artifact'
const CREATE_ARTIFACT_COMMAND = 'create_artifact'
const UPDATE_ARTIFACT_COMMAND = 'update_artifact'
const DELETE_ARTIFACT_COMMAND = 'delete_artifact'
const SET_ARTIFACT_CHECK_COMMAND = 'set_artifact_check'
const SET_ARTIFACT_CRITERION_COMMAND = 'set_artifact_criterion'
const UPDATE_ITEM_PLAN_COMMAND = 'update_item_plan'
const IMPORT_PLAN_COMMAND = 'import_plan'
const SAVE_PLAN_MARKDOWN_COMMAND = 'save_plan_markdown'
const PICK_PLAN_FILE_COMMAND = 'pick_plan_file'
const PLAN_ARTIFACT_PREFIX = 'plano-'

// O backend só devolve texto; o conflito de edição é reconhecido por este trecho da mensagem (DbError::ArtifactConflict).
const CONFLICT_MESSAGE_MARKER = 'foi alterado depois da sua leitura'
// Mesmo motivo, para DbError::PlanStageStarted: o id da etapa vem no texto.
const STAGE_STARTED_PATTERN = /A etapa (\S+) já foi iniciada/

export const PLAN_KIND = 'plano'

export type ArtifactKindId = 'documento' | 'especificacao' | 'checklist' | 'arquitetura' | 'decisao'

export interface ArtifactKindInfo {
  kind: ArtifactKindId
  label: string
  description: string
  template: string
}

export interface ArtifactSummary {
  id: string
  kind: ArtifactKindId | typeof PLAN_KIND
  title: string
  summary: string
  tags: string[]
  author?: string
  itemId?: string
  updatedAt: number
}

export interface ArtifactDocument {
  id: string
  kind: ArtifactKindId
  title: string
  summary: string
  body: string
  tags: string[]
  author?: string
  createdAt: number
  updatedAt: number
}

export interface ArtifactCriterion {
  id: string
  text: string
  done: boolean
}

export interface PlanArtifactDetail {
  id: string
  itemId: string
  title: string
  summary: string
  points: string[]
  estimate: string
  risks: string[]
  tags: string[]
  detail: string
  author?: string
  criteria: ArtifactCriterion[]
  createdAt: number
  updatedAt: number
}

export interface PlanFile {
  name: string
  text: string
}

export interface NewArtifactInput {
  kind: ArtifactKindId
  title: string
  summary?: string
  body?: string
  tags?: string[]
  author?: string
}

export interface ArtifactPatch {
  title?: string
  summary?: string
  body?: string
  tags?: string[]
  expectedUpdatedAt?: number
}

export function fetchArtifactKinds(): Promise<ArtifactKindInfo[]> {
  return invoke<ArtifactKindInfo[]>(LIST_ARTIFACT_KINDS_COMMAND)
}

export function fetchArtifacts(projectId: number): Promise<ArtifactSummary[]> {
  return invoke<ArtifactSummary[]>(LIST_ARTIFACTS_COMMAND, { projectId })
}

export function planArtifactId(itemId: string): string {
  return `${PLAN_ARTIFACT_PREFIX}${itemId}`
}

export function fetchArtifact(projectId: number, id: string): Promise<ArtifactDocument> {
  return invoke<ArtifactDocument>(GET_ARTIFACT_COMMAND, { projectId, id })
}

// O mesmo get_artifact devolve o plano com os critérios persistidos quando o id é plano-<card>.
export function fetchPlanArtifact(projectId: number, itemId: string): Promise<PlanArtifactDetail> {
  return invoke<PlanArtifactDetail>(GET_ARTIFACT_COMMAND, { projectId, id: planArtifactId(itemId) })
}

export function createArtifact(projectId: number, document: NewArtifactInput): Promise<ArtifactDocument> {
  return invoke<ArtifactDocument>(CREATE_ARTIFACT_COMMAND, { projectId, document })
}

export function updateArtifact(projectId: number, id: string, patch: ArtifactPatch): Promise<ArtifactDocument> {
  return invoke<ArtifactDocument>(UPDATE_ARTIFACT_COMMAND, { projectId, id, patch })
}

export function deleteArtifact(projectId: number, id: string): Promise<void> {
  return invoke<void>(DELETE_ARTIFACT_COMMAND, { projectId, id })
}

export function setArtifactCheck(projectId: number, id: string, index: number, done: boolean): Promise<ArtifactDocument> {
  return invoke<ArtifactDocument>(SET_ARTIFACT_CHECK_COMMAND, { projectId, id, index, done })
}

export function setArtifactCriterion(
  projectId: number,
  artifactId: string,
  criterionId: string,
  done: boolean,
): Promise<void> {
  return invoke<void>(SET_ARTIFACT_CRITERION_COMMAND, { projectId, artifactId, criterionId, done })
}

export function updateItemPlan(
  projectId: number,
  itemId: string,
  proposal: string,
  expectedUpdatedAt?: number,
): Promise<PlanArtifactDetail> {
  return invoke<PlanArtifactDetail>(UPDATE_ITEM_PLAN_COMMAND, { projectId, itemId, proposal, expectedUpdatedAt })
}

export function importPlan(projectId: number, title: string, proposal: string): Promise<BacklogItem> {
  return invoke<BacklogItem>(IMPORT_PLAN_COMMAND, { projectId, title, proposal })
}

// Devolve o caminho gravado, ou null quando o usuário cancela o diálogo.
export function savePlanMarkdown(suggestedName: string, content: string): Promise<string | null> {
  return invoke<string | null>(SAVE_PLAN_MARKDOWN_COMMAND, { suggestedName, content })
}

export function pickPlanFile(): Promise<PlanFile | null> {
  return invoke<PlanFile | null>(PICK_PLAN_FILE_COMMAND)
}

export function startedStageOf(message: string): string | undefined {
  return STAGE_STARTED_PATTERN.exec(message)?.[1]
}

export function isArtifactConflict(message: string): boolean {
  return message.includes(CONFLICT_MESSAGE_MARKER)
}
