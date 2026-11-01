import { computed, onScopeDispose, ref } from 'vue'

const COUNTDOWN_TICK_MS = 1000
const MS_PER_SECOND = 1000
const SECONDS_PER_MINUTE = 60

export function usePermissionCountdown(expiresAt: number) {
  const now = ref(Date.now())
  const timer = setInterval(() => {
    now.value = Date.now()
  }, COUNTDOWN_TICK_MS)
  onScopeDispose(() => clearInterval(timer))

  const label = computed(() => {
    const seconds = Math.max(0, Math.ceil((expiresAt - now.value) / MS_PER_SECOND))
    const minutes = Math.floor(seconds / SECONDS_PER_MINUTE)
    return `${String(minutes).padStart(2, '0')}:${String(seconds % SECONDS_PER_MINUTE).padStart(2, '0')}`
  })

  return { label }
}
