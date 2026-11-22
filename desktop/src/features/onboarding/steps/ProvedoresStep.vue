<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import UiButton from '../../../ui/UiButton.vue'
import UiNotice from '../../../ui/UiNotice.vue'
import UiSegmented from '../../../ui/UiSegmented.vue'
import UiSelect from '../../../ui/UiSelect.vue'
import type { TestProviderResult } from '../../providers/providersApi'
import { useProviders } from '../../providers/useProviders'
import type { ModelTier } from '../../team/teamApi'
import { useOnboardingLlm } from '../useOnboardingLlm'
import StepHeading from './StepHeading.vue'

const EFFORT_OPTIONS: readonly { label: string; value: ModelTier }[] = [
  { label: 'Básico', value: 'basic' },
  { label: 'Intermediário', value: 'intermediate' },
  { label: 'Avançado', value: 'advanced' },
]

const providersStore = useProviders()
const onboardingLlm = useOnboardingLlm()

const testingId = ref<string | null>(null)
const testResults = ref<Record<string, TestProviderResult | null>>({})

const installedProviders = computed(() => providersStore.providers.filter((p) => p.installed))
const providerOptions = computed(() =>
  providersStore.providers.map((p) => ({
    label: `${p.label}${p.installed ? '' : ' (ausente)'}`,
    value: p.id,
  })),
)
const fallbackOptions = computed(() => [
  { label: 'Nenhum', value: '' },
  ...providersStore.providers
    .filter((p) => p.id !== onboardingLlm.defaultCli)
    .map((p) => ({ label: p.label, value: p.id })),
])

onMounted(async () => {
  try {
    await providersStore.load()
  } catch {
    // erro já registrado em providersStore.error
  }
  const preferredIds = installedProviders.value.length > 0
    ? installedProviders.value.map((p) => p.id)
    : providersStore.providers.map((p) => p.id)
  await onboardingLlm.initFromSettings(preferredIds)
})

watch(
  () => providersStore.providers,
  (list) => {
    if (!onboardingLlm.defaultCli && list.length > 0) {
      const firstInstalled = list.find((p) => p.installed)
      onboardingLlm.defaultCli = (firstInstalled ?? list[0]).id
    }
  },
)

async function testProvider(id: string): Promise<void> {
  const provider = providersStore.providers.find((p) => p.id === id)
  if (!provider) return
  testingId.value = id
  try {
    const model = provider.tiers[onboardingLlm.effortLevel] || provider.defaultModel || undefined
    const result = await providersStore.testConfig(provider, model)
    testResults.value = { ...testResults.value, [id]: result }
  } catch (err) {
    testResults.value = {
      ...testResults.value,
      [id]: { ok: false, latencyMs: 0, reply: '', errorKind: 'other', errorMessage: String(err) },
    }
  } finally {
    testingId.value = null
  }
}
</script>

<template>
  <div data-testid="onboarding-providers-step" class="flex w-full flex-col gap-6">
    <StepHeading
      title="Conecte suas LLMs"
      text="Escolha seu provedor padrão ou adicione novas CLIs via auto-setup."
    />

    <UiNotice
      v-if="!providersStore.loading && installedProviders.length === 0"
      text="Nenhuma CLI detectada ainda. Você pode configurar agora ou avançar e ajustar depois em Configurações ou /setup."
    />

    <div class="flex flex-col gap-4 rounded-md border border-line bg-surface p-5">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <span class="text-control font-semibold text-ink">Preferência inicial</span>
        <UiButton
          data-testid="onboarding-open-setup"
          variant="quiet"
          size="sm"
          @press="providersStore.openSetup()"
        >
          Configurar / Adicionar LLM
        </UiButton>
      </div>

      <div v-if="providerOptions.length > 0" class="flex flex-col gap-3.5">
        <label class="flex flex-col gap-1.5">
          <span class="text-label font-medium text-muted">Provedor Padrão</span>
          <UiSelect
            v-model="onboardingLlm.defaultCli"
            data-testid="onboarding-default-provider"
            :options="providerOptions"
            label="Provedor Padrão"
          />
        </label>

        <div class="flex flex-col gap-1.5">
          <span class="text-label font-medium text-muted">Nível de Esforço/Inteligência</span>
          <UiSegmented
            v-model="onboardingLlm.effortLevel"
            data-testid="onboarding-effort-level"
            :options="EFFORT_OPTIONS"
            label="Nível de Esforço/Inteligência"
          />
        </div>

        <label v-if="providersStore.providers.length >= 2" class="flex flex-col gap-1.5">
          <span class="text-label font-medium text-muted">Provedor Fallback</span>
          <UiSelect
            v-model="onboardingLlm.fallbackCli"
            data-testid="onboarding-fallback-provider"
            :options="fallbackOptions"
            label="Provedor Fallback"
          />
        </label>
      </div>
    </div>

    <ul
      v-if="providersStore.providers.length > 0"
      class="flex flex-col divide-y divide-line rounded-md border border-line bg-surface"
    >
      <li
        v-for="provider in providersStore.providers"
        :key="provider.id"
        class="flex flex-col gap-1 px-4 py-2.5"
      >
        <div class="flex items-center justify-between gap-2">
          <div class="flex min-w-0 items-center gap-2">
            <span
              class="size-2 shrink-0 rounded-full"
              :class="provider.installed ? 'bg-ok' : 'bg-faint'"
              aria-hidden="true"
            />
            <span class="truncate text-caption font-medium text-ink">{{ provider.label }}</span>
            <span
              class="inline-flex h-5 items-center rounded-sm px-2 text-small font-medium"
              :class="provider.installed ? 'bg-ok-soft text-ok' : 'bg-surface-2 text-muted'"
            >
              {{ provider.installed ? 'Instalado' : 'Ausente' }}
            </span>
          </div>
          <div class="flex shrink-0 items-center gap-1">
            <UiButton
              variant="quiet"
              size="sm"
              :disabled="testingId === provider.id"
              @press="testProvider(provider.id)"
            >
              {{ testingId === provider.id ? 'Testando...' : 'Testar' }}
            </UiButton>
            <UiButton variant="quiet" size="sm" @press="providersStore.openSetup(provider.id)">
              Editar
            </UiButton>
          </div>
        </div>
        <p
          v-if="testResults[provider.id]"
          class="text-small"
          :class="testResults[provider.id]!.ok ? 'text-ok' : 'text-danger'"
        >
          {{
            testResults[provider.id]!.ok
              ? `OK (${testResults[provider.id]!.latencyMs} ms)`
              : testResults[provider.id]!.errorMessage ?? 'Falha no teste'
          }}
        </p>
      </li>
    </ul>
  </div>
</template>
