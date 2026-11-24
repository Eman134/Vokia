import { ref } from 'vue'
import { defineStore } from 'pinia'
import { useBalloons } from '../balloon/useBalloons'
import type { VokiaExecutionModeChanged } from '../team/events'
import { fetchExecutionMode, saveExecutionMode } from './executionModeApi'
import { DEFAULT_EXECUTION_MODE, EXECUTION_MODE_INFO, type ExecutionMode } from './executionModes'

// A chave antiga continua valendo: quem já aceitou o aviso do bypass não o vê de novo.
const WARNING_ACK_KEY = 'vokia.bypass.warning-acknowledged'
const CHANGED_TITLE_PREFIX = 'Modo de execução:'

function readAcknowledged(): boolean {
  try {
    return localStorage.getItem(WARNING_ACK_KEY) === '1'
  } catch {
    return false
  }
}

// Estado só em memória no backend, por projeto: o app abre sempre no modo manual.
export const useExecutionMode = defineStore('execution-mode', () => {
  const modeByProject = ref<Record<number, ExecutionMode>>({})
  const warningAcknowledged = ref(readAcknowledged())
  const error = ref<string | null>(null)

  function modeOf(projectId: number | null | undefined): ExecutionMode {
    return (projectId != null && modeByProject.value[projectId]) || DEFAULT_EXECUTION_MODE
  }

  async function load(projectId: number): Promise<void> {
    try {
      modeByProject.value[projectId] = await fetchExecutionMode(projectId)
    } catch (failure) {
      error.value = `Não foi possível ler o modo de execução: ${String(failure)}`
    }
  }

  function acknowledgeWarning(): void {
    warningAcknowledged.value = true
    try {
      localStorage.setItem(WARNING_ACK_KEY, '1')
    } catch {
      // Sem armazenamento, o aviso volta na próxima sessão.
    }
  }

  async function set(projectId: number, mode: ExecutionMode): Promise<void> {
    try {
      await saveExecutionMode(projectId, mode)
      error.value = null
    } catch (failure) {
      error.value = `Não foi possível mudar para o modo ${EXECUTION_MODE_INFO[mode].label}: ${String(failure)}`
    }
  }

  // O evento do backend é a fonte da verdade e o único que dispara o balão: vale para qualquer origem da mudança.
  function apply(event: VokiaExecutionModeChanged): void {
    const changed = modeOf(event.projectId) !== event.mode
    modeByProject.value[event.projectId] = event.mode
    if (changed) {
      const info = EXECUTION_MODE_INFO[event.mode]
      useBalloons().alert(`${CHANGED_TITLE_PREFIX} ${info.label}`, info.description)
    }
  }

  return { modeByProject, warningAcknowledged, error, modeOf, load, set, acknowledgeWarning, apply }
})
