import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { BalloonItem } from '../balloon/balloonTypes'
import { useBalloons } from '../balloon/useBalloons'
import type { VokiaUsageLimit } from '../team/events'
import { availableProviders, resumeInterruptedRun, type AgentProfile, type CliPreset } from '../team/teamApi'
import { useFeed } from '../team/useFeed'
import { useRoster } from '../team/useRoster'
import { useUsage } from './useUsage'
import { MS_PER_SECOND, resetCountdownLabel } from './usagePlans'

const MIGRATION_PRIORITY = ['codex', 'antigravity', 'opencode', 'claude']

export const useUsageLimit = defineStore('usageLimit', () => {
  const usage = useUsage()
  const feed = useFeed()
  const roster = useRoster()
  const balloons = useBalloons()
  const notice = ref<VokiaUsageLimit | null>(null)
  const visible = computed(
    () =>
      notice.value !== null &&
      (notice.value.resetsAt === 0 || usage.now < notice.value.resetsAt * MS_PER_SECOND),
  )
  const resumable = computed(() => notice.value?.runId != null)

  function enqueueOrReplace(item: BalloonItem): void {
    if (!balloons.replace(item.id, item)) {
      balloons.enqueue(item)
    }
  }

  function pickAlternativeProvider(currentCli: string, installed: readonly string[]): CliPreset | undefined {
    const ordered = [
      ...MIGRATION_PRIORITY.filter((id) => id !== currentCli && installed.includes(id)),
      ...installed.filter((id) => id !== currentCli && !MIGRATION_PRIORITY.includes(id)),
    ]
    for (const id of ordered) {
      const preset = roster.presets.find((item) => item.id === id)
      if (preset !== undefined) {
        return preset
      }
    }
    return undefined
  }

  async function migrateAndContinue(
    profile: AgentProfile,
    target: CliPreset,
    runId: number | null,
  ): Promise<string | void> {
    try {
      await roster.update(profile.id, {
        name: profile.name,
        role: profile.role,
        avatar: profile.avatar,
        cli: target.id,
        model: target.defaultModel,
        command: target.defaultCommand,
      })
      if (runId !== null) {
        await resume(runId)
      }
      return `Migrado para ${target.label}`
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
    }
  }

  async function suggestMigration(event: VokiaUsageLimit): Promise<void> {
    const balloonId = `limit-exhausted:${event.agent}`
    const profile = roster.profileOf(event.agent)
    const when = event.resetsAt > 0 ? resetCountdownLabel(event.resetsAt, usage.now) : null
    if (profile === undefined || profile.builtin) {
      if (when !== null) {
        enqueueOrReplace({
          id: balloonId,
          kind: 'limit',
          source: 'uso do plano',
          title: 'Limite esgotado',
          text: `O limite volta ${when}.`,
          mascot: 'pausado',
          actions: [{ id: 'wait', label: 'Esperar', primary: true }],
        })
      }
      return
    }
    const installed = await availableProviders().catch(() => [] as string[])
    const target = pickAlternativeProvider(profile.cli, installed)
    if (target === undefined) {
      if (when !== null) {
        enqueueOrReplace({
          id: balloonId,
          kind: 'limit',
          source: 'uso do plano',
          title: 'Limite esgotado',
          text: `O limite volta ${when}.`,
          mascot: 'pausado',
          actions: [{ id: 'wait', label: 'Esperar', primary: true }],
        })
      }
      return
    }
    const text =
      when !== null
        ? `O limite volta ${when}. Deseja migrar ${profile.name} para ${target.label} e continuar?`
        : `Deseja migrar ${profile.name} para ${target.label} e continuar?`
    enqueueOrReplace({
      id: balloonId,
      kind: 'limit',
      source: 'uso do plano',
      title: 'Limite esgotado',
      text,
      mascot: 'pausado',
      actions: [
        {
          id: 'migrate',
          label: `Migrar para ${target.label} e continuar`,
          primary: true,
          run: () => migrateAndContinue(profile, target, event.runId),
        },
        { id: 'wait', label: 'Esperar' },
      ],
    })
  }

  function apply(event: VokiaUsageLimit): void {
    usage.refreshClock()
    notice.value = event
    void suggestMigration(event)
  }

  function suggestFromLimitMessage(agent: string): void {
    if (notice.value?.agent === agent && visible.value) {
      return
    }
    const event: VokiaUsageLimit = {
      type: 'usage-limit',
      agent,
      projectId: 0,
      window: 'five-hour',
      resetsAt: 0,
      runId: null,
    }
    notice.value = event
    void suggestMigration(event)
  }

  async function resume(targetRunId?: number | null): Promise<void> {
    const runId = targetRunId ?? notice.value?.runId
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

  return { notice, visible, resumable, apply, suggestFromLimitMessage, resume }
})
