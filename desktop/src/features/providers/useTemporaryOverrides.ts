import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { Agent, TemporaryOverride } from '../team/events'
import {
  clearAllTemporaryOverrides,
  clearTemporaryOverride,
  listTemporaryOverrides,
} from '../team/teamApi'

export const useTemporaryOverrides = defineStore('temporary-overrides', () => {
  const overrides = ref<TemporaryOverride[]>([])
  const restoring = ref<Record<string, boolean>>({})

  async function load(): Promise<void> {
    overrides.value = await listTemporaryOverrides()
  }

  function apply(next: TemporaryOverride[]): void {
    overrides.value = next
  }

  function byAgent(id: Agent): TemporaryOverride | undefined {
    const now = Date.now()
    return overrides.value.find(
      (item) => item.agent === id && (item.expiresAt == null || item.expiresAt > now),
    )
  }

  async function restore(agentId: Agent): Promise<void> {
    if (restoring.value[agentId]) return
    restoring.value = { ...restoring.value, [agentId]: true }
    try {
      await clearTemporaryOverride(agentId)
      overrides.value = overrides.value.filter((item) => item.agent !== agentId)
    } finally {
      const next = { ...restoring.value }
      delete next[agentId]
      restoring.value = next
    }
  }

  async function restoreAll(): Promise<void> {
    await clearAllTemporaryOverrides()
    overrides.value = []
  }

  return {
    overrides,
    restoring,
    load,
    apply,
    byAgent,
    restore,
    restoreAll,
  }
})
