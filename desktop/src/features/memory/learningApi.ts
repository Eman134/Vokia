import { invoke } from '@tauri-apps/api/core'

export type SuggestionKind = 'memory' | 'skill'
export type SuggestionOperation = 'add' | 'update' | 'forget' | 'create'
export type SuggestionStatus = 'pending' | 'applied' | 'dismissed'
export type SkillScope = 'project' | 'global'
export type MemoryOriginType = 'manual' | 'pinned' | 'review' | 'auto'

export interface LearningSuggestion {
  id: string
  projectId: number
  kind: SuggestionKind
  operation: SuggestionOperation
  status: SuggestionStatus
  targetMemoryId?: string
  skillSlug?: string
  skillScope: SkillScope
  title: string
  content: string
  previousContent?: string
  rationale: string
  sourceAgent?: string
  sourceCardId?: string
  providerId?: string
  model?: string
  createdAt: number
  updatedAt: number
  resolvedAt?: number
}

export interface ProjectMemory {
  id: string
  projectId: number
  fact: string
  source: string
  originType: MemoryOriginType
  pinned: boolean
  author?: string
  createdAt: number
  updatedAt: number
}

export interface ReviewOutcome {
  status: string
  createdSuggestions: number
  autoApplied: number
  providerId?: string
  model?: string
}

export function listLearningSuggestions(
  projectId?: number,
  status?: SuggestionStatus,
): Promise<LearningSuggestion[]> {
  return invoke<LearningSuggestion[]>('list_learning_suggestions', {
    args: { projectId, status },
  })
}

export function applyLearningSuggestion(
  id: string,
  scope?: SkillScope,
): Promise<LearningSuggestion> {
  return invoke<LearningSuggestion>('apply_learning_suggestion', {
    args: { id, scope },
  })
}

export function dismissLearningSuggestion(id: string): Promise<LearningSuggestion> {
  return invoke<LearningSuggestion>('dismiss_learning_suggestion', {
    args: { id },
  })
}

export function runMemoryReview(projectId: number): Promise<ReviewOutcome> {
  return invoke<ReviewOutcome>('run_memory_review', {
    args: { projectId, trigger: 'manual' },
  })
}

export function listProjectMemories(projectId: number): Promise<ProjectMemory[]> {
  return invoke<ProjectMemory[]>('list_project_memories', {
    args: { projectId },
  })
}
