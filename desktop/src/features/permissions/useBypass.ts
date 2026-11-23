import { ref } from 'vue'
import { defineStore } from 'pinia'
import { useBalloons } from '../balloon/useBalloons'
import type { VokiaBypassChanged } from '../team/events'
import { fetchBypassMode, saveBypassMode } from './bypassApi'

const WARNING_ACK_KEY = 'vokia.bypass.warning-acknowledged'
const ON_TITLE = 'Modo bypass ligado'
const ON_TEXT = 'Os pedidos de permissão deste projeto são aprovados sozinhos. Comandos destrutivos ainda pedem a sua decisão.'
const OFF_TITLE = 'Modo bypass desligado'
const OFF_TEXT = 'Os pedidos de permissão voltam a esperar pela sua decisão.'

function readAcknowledged(): boolean {
  try {
    return localStorage.getItem(WARNING_ACK_KEY) === '1'
  } catch {
    return false
  }
}

// Estado só em memória no backend, por projeto: o app abre sempre com o bypass desligado.
export const useBypass = defineStore('bypass', () => {
  const enabledByProject = ref<Record<number, boolean>>({})
  const warningAcknowledged = ref(readAcknowledged())
  const error = ref<string | null>(null)

  function isOn(projectId: number | null | undefined): boolean {
    return projectId != null && enabledByProject.value[projectId] === true
  }

  async function load(projectId: number): Promise<void> {
    try {
      enabledByProject.value[projectId] = await fetchBypassMode(projectId)
    } catch (failure) {
      error.value = `Não foi possível ler o modo bypass: ${String(failure)}`
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

  async function set(projectId: number, enabled: boolean): Promise<void> {
    try {
      await saveBypassMode(projectId, enabled)
      error.value = null
    } catch (failure) {
      error.value = `Não foi possível ${enabled ? 'ligar' : 'desligar'} o modo bypass: ${String(failure)}`
    }
  }

  // O evento do backend é a fonte da verdade e o único que dispara o toast: vale para qualquer origem da mudança.
  function apply(event: VokiaBypassChanged): void {
    const changed = isOn(event.projectId) !== event.enabled
    enabledByProject.value[event.projectId] = event.enabled
    if (changed) {
      useBalloons().alert(event.enabled ? ON_TITLE : OFF_TITLE, event.enabled ? ON_TEXT : OFF_TEXT)
    }
  }

  return { enabledByProject, warningAcknowledged, error, isOn, load, set, acknowledgeWarning, apply }
})
