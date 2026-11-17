import { computed, onScopeDispose, ref } from 'vue'
import { defineStore } from 'pinia'

// Abaixo de 640 px o trilho aberto deixaria menos de 160 px para a tela: nessa largura ele só existe recolhido.
const NARROW_QUERY = '(max-width: 639.98px)'

export const useRail = defineStore('rail', () => {
  const requested = ref(false)
  const narrowMedia = window.matchMedia(NARROW_QUERY)
  const narrow = ref(narrowMedia.matches)

  function onNarrowChange(event: MediaQueryListEvent): void {
    narrow.value = event.matches
  }

  narrowMedia.addEventListener('change', onNarrowChange)
  onScopeDispose(() => narrowMedia.removeEventListener('change', onNarrowChange))

  const canExpand = computed(() => !narrow.value)
  const expanded = computed({
    get: () => requested.value && canExpand.value,
    set: (value: boolean) => {
      requested.value = value
    },
  })

  function toggle(): void {
    requested.value = !requested.value
  }

  return { expanded, canExpand, toggle }
})
