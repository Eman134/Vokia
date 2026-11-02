import { nextTick, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { useNavigation } from '../../shell/useNavigation'
import { DEFAULT_PROJECT_ICON } from './projectIcons'
import {
  createProject,
  inspectFolder,
  inspectGithubRepo,
  isProjectError,
  pickFolder,
  type CloneMode,
  type NewProjectInput,
  type OriginKind,
  type ProjectErrorCode,
} from './projectsApi'
import { useProjects } from './useProjects'

export type WizardStep = 1 | 2 | 3
export type ErrorField = 'path' | 'repo' | 'parentDir' | 'branch' | 'symbol' | 'general'

export interface WizardError {
  field: ErrorField
  message: string
}

const PARENT_REQUIRED_MESSAGE = 'Escolha a pasta de destino.'
const FIRST_STEP: WizardStep = 1
const LAST_STEP: WizardStep = 3

const ERROR_FIELD: Record<ProjectErrorCode, ErrorField> = {
  GitNotInstalled: 'path',
  FolderNotFound: 'path',
  NotADirectory: 'path',
  GhNotInstalled: 'repo',
  GhNotAuthenticated: 'repo',
  InvalidRepoRef: 'repo',
  RepoNotFound: 'repo',
  GhFailed: 'repo',
  DestinationExists: 'parentDir',
  BranchNotFound: 'branch',
  CloneFailed: 'general',
  CloneTimeout: 'general',
  ProjectAlreadyAdded: 'general',
  CreateInProgress: 'general',
  InvalidIcon: 'symbol',
  ProjectNotFound: 'general',
  ProjectBusy: 'general',
  Db: 'general',
}

export const useNovoProjeto = defineStore('newProject', () => {
  const navigation = useNavigation()
  const projects = useProjects()

  const step = ref<WizardStep>(FIRST_STEP)
  const originKind = ref<OriginKind>('folder')
  const path = ref('')
  const repo = ref('')
  const mode = ref<CloneMode>('link')
  const parentDir = ref('')
  const branch = ref('')
  const linkedRepo = ref<string | null>(null)
  const name = ref('')
  const symbol = ref<string>(DEFAULT_PROJECT_ICON)
  const description = ref('')
  const nameEdited = ref(false)
  const busy = ref(false)
  const error = ref<WizardError | null>(null)

  watch([path, repo, parentDir, branch], () => {
    error.value = null
  })

  watch(path, () => {
    linkedRepo.value = null
  })

  function messageOf(cause: unknown): string {
    return isProjectError(cause) ? cause.message : String(cause)
  }

  async function choosePath() {
    const picked = await pickFolder()
    if (picked !== null) path.value = picked
  }

  async function chooseParent() {
    const picked = await pickFolder()
    if (picked !== null) parentDir.value = picked
  }

  function applyDefaults(detectedName: string) {
    if (!nameEdited.value) name.value = detectedName
  }

  async function inspect() {
    if (busy.value) return
    busy.value = true
    error.value = null
    try {
      if (originKind.value === 'folder') {
        const info = await inspectFolder(path.value.trim())
        path.value = info.path
        await nextTick()
        linkedRepo.value = info.githubRepo
        applyDefaults(info.name)
      } else {
        linkedRepo.value = null
        if (parentDir.value.trim() === '') {
          error.value = { field: 'parentDir', message: PARENT_REQUIRED_MESSAGE }
          return
        }
        const info = await inspectGithubRepo(repo.value.trim())
        applyDefaults(info.name)
      }
      step.value = 2
    } catch (cause) {
      const field = isProjectError(cause) ? (ERROR_FIELD[cause.code] ?? 'general') : 'general'
      error.value = { field, message: messageOf(cause) }
    } finally {
      busy.value = false
    }
  }

  function setName(value: string) {
    name.value = value
    nameEdited.value = true
  }

  function setSymbol(value: string) {
    symbol.value = value
  }

  function next() {
    if (step.value < LAST_STEP) step.value = (step.value + 1) as WizardStep
  }

  function reset() {
    step.value = FIRST_STEP
    originKind.value = 'folder'
    path.value = ''
    repo.value = ''
    mode.value = 'link'
    parentDir.value = ''
    branch.value = ''
    linkedRepo.value = null
    name.value = ''
    symbol.value = DEFAULT_PROJECT_ICON
    description.value = ''
    nameEdited.value = false
    busy.value = false
    error.value = null
  }

  function back() {
    if (step.value === FIRST_STEP) {
      reset()
      navigation.goTo('projetos')
      return
    }
    step.value = (step.value - 1) as WizardStep
  }

  function buildInput(): NewProjectInput {
    const base = { name: name.value, symbol: symbol.value, description: description.value }
    if (originKind.value === 'folder') {
      return { ...base, origin: { kind: 'folder', path: path.value } }
    }
    return {
      ...base,
      origin: {
        kind: 'github',
        repo: repo.value.trim(),
        mode: mode.value,
        parentDir: parentDir.value,
        branch: branch.value.trim() || null,
      },
    }
  }

  async function create() {
    if (busy.value) return
    busy.value = true
    error.value = null
    try {
      await createProject(buildInput())
      await projects.load()
      reset()
      navigation.goTo('projetos')
    } catch (cause) {
      error.value = { field: 'general', message: messageOf(cause) }
    } finally {
      busy.value = false
    }
  }

  return {
    step, originKind, path, repo, mode, parentDir, branch, linkedRepo, name, symbol, description, busy, error,
    choosePath, chooseParent, inspect, setName, setSymbol, next, back, create, reset,
  }
})
