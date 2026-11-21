import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { BalloonItem } from '../balloon/balloonTypes'
import { useBalloons } from '../balloon/useBalloons'
import type { VokiaModelUnavailable, VokiaUsageLimit } from '../team/events'
import { availableProviders, resumeInterruptedRun, type AgentProfile, type CliPreset } from '../team/teamApi'
import { useFeed } from '../team/useFeed'
import { useRoster } from '../team/useRoster'
import { useActiveLlm } from '../providers/useActiveLlm'
import { useProviders } from '../providers/useProviders'
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
    resetsAt: number,
  ): Promise<string | void> {
    try {
      const resolvedModel =
        (profile.modelTier ? target.tiers[profile.modelTier] : '') || target.defaultModel
      const expiresInMs =
        resetsAt > 0 ? Math.max(60_000, resetsAt * MS_PER_SECOND - Date.now()) : null
      await useActiveLlm().applyOverride({
        agent: profile.id,
        cli: target.id,
        modelTier: profile.modelTier ?? null,
        model: resolvedModel,
        reason: 'usage-limit',
        expiresInMs,
      })
      if (runId !== null) {
        await resume(runId)
      }
      return `Continuando temporariamente com ${target.label}`
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
          label: `Continuar temporariamente com ${target.label}`,
          primary: true,
          run: () => migrateAndContinue(profile, target, event.runId, event.resetsAt),
        },
        { id: 'wait', label: 'Esperar' },
      ],
    })
  }

  function suggestModelUnavailable(event: VokiaModelUnavailable): void {
    const profile = roster.profileOf(event.agent)
    const preset = roster.presets.find((item) => item.id === event.provider)
    const fallbackModel =
      preset?.models.find((m) => m !== event.model) ??
      preset?.tiers.intermediate ??
      preset?.defaultModel ??
      null

    const actions: BalloonItem['actions'] = []
    if (profile && preset && fallbackModel && fallbackModel !== event.model) {
      actions.push({
        id: 'use-available-model',
        label: 'Usar modelo disponível temporariamente',
        primary: true,
        run: async () => {
          await useActiveLlm().applyOverride({
            agent: profile.id,
            cli: preset.id,
            modelTier: null,
            model: fallbackModel,
            reason: 'model-unavailable',
          })
          return `Usando temporariamente ${fallbackModel}`
        },
      })
    }
    actions.push({
      id: 'open-setup',
      label: 'Abrir Setup',
      run: () => {
        useProviders().openSetup(event.provider)
      },
    })

    enqueueOrReplace({
      id: `model-unavailable:${event.agent}`,
      kind: 'limit',
      source: 'uso do plano',
      title: 'Modelo sem acesso',
      text: `${event.provider} recusou o modelo "${event.model}" (${event.detail}).`,
      mascot: 'pausado',
      actions,
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

  return { notice, visible, resumable, apply, suggestFromLimitMessage, suggestModelUnavailable, resume }
})
