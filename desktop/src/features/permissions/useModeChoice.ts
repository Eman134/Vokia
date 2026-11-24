import { ref, type Ref } from 'vue'
import type { ExecutionMode } from './executionModes'
import { useExecutionMode } from './useExecutionMode'

// Entrar em bypass passa pelo aviso até o usuário aceitá-lo uma vez; os outros modos mudam direto.
export function useModeChoice(projectId: Ref<number | null>) {
  const store = useExecutionMode()
  const warningOpen = ref(false)

  function choose(mode: ExecutionMode): void {
    if (projectId.value === null || mode === store.modeOf(projectId.value)) return
    if (mode === 'bypass' && !store.warningAcknowledged) {
      warningOpen.value = true
      return
    }
    void store.set(projectId.value, mode)
  }

  function acceptWarning(): void {
    store.acknowledgeWarning()
    choose('bypass')
  }

  return { warningOpen, choose, acceptWarning }
}
