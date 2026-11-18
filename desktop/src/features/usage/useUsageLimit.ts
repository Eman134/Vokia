import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { VokiaUsageLimit } from '../team/events'
import { resumeInterruptedRun } from '../team/teamApi'
import { useFeed } from '../team/useFeed'
import { useUsage } from './useUsage'
import { MS_PER_SECOND } from './usagePlans'

export const useUsageLimit = defineStore('usageLimit', () => {
  const usage = useUsage()
  const feed = useFeed()
  const notice = ref<VokiaUsageLimit | null>(null)
  const visible = computed(
    () => notice.value !== null && usage.now < notice.value.resetsAt * MS_PER_SECOND,
  )
  const resumable = computed(() => notice.value?.runId != null)

  function apply(event: VokiaUsageLimit): void {
    usage.refreshClock()
    notice.value = event
  }

  async function resume(): Promise<void> {
    const runId = notice.value?.runId
    if (runId === null || runId === undefined) {
      return
    }
    notice.value = null
    try {
      await resumeInterruptedRun(runId)
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
    }
  }

  return { notice, visible, resumable, apply, resume }
})
