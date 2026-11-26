<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'
import UiButton from '../../../ui/UiButton.vue'
import UiNotice from '../../../ui/UiNotice.vue'
import UiSegmented from '../../../ui/UiSegmented.vue'
import UiSelect from '../../../ui/UiSelect.vue'
import ProviderConnectionCard from '../../providers/ProviderConnectionCard.vue'
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

const connectedProviders = computed(() =>
  providersStore.providers.filter(
    (p) =>
      p.enabled !== false &&
      (p.diagnosticState === 'connected' || p.connectionStatus === 'connected'),
  ),
)

const providerOptions = computed(() => [
  { label: 'Escolher depois', value: '' },
  ...providersStore.providers.map((p) => ({
    label: `${p.label}${p.diagnosticState === 'connected' || p.connectionStatus === 'connected' ? '' : p.installed ? ' (sem login)' : ' (não instalado)'}`,
    value: p.id,
  })),
])

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
  await onboardingLlm.initFromSettings(connectedProviders.value.map((p) => p.id))
})

watch(
  connectedProviders,
  (list) => {
    if (!onboardingLlm.defaultCli && list.length > 0) {
      onboardingLlm.defaultCli = list[0].id
    }
  },
)

function selectProvider(id: string): void {
  onboardingLlm.defaultCli = id
}
</script>

<template>
  <div data-testid="onboarding-providers-step" class="flex w-full flex-col gap-6">
    <StepHeading
      title="Conecte suas LLMs"
      text="Escolha livremente qualquer provedor para conectar (Claude, Codex, OpenCode, Gemini ou customizado) e avance assim que quiser."
    />

    <UiNotice
      v-if="!providersStore.loading && connectedProviders.length === 0"
      text="Nenhum provedor conectado ainda. Conecte qualquer provedor abaixo (ou ative o modo gratuito do OpenCode) ou avance para configurar depois."
    />

    <div class="flex flex-wrap items-center justify-between gap-2">
      <span class="text-label font-medium text-muted">
        {{ connectedProviders.length }} de {{ providersStore.providers.length }} conectados
      </span>
      <UiButton
        data-testid="onboarding-open-setup"
        variant="quiet"
        size="sm"
        @press="providersStore.openAddProvider()"
      >
        + Adicionar provedor customizado
      </UiButton>
    </div>

    <div class="flex flex-col gap-3">
      <ProviderConnectionCard
        v-for="provider in providersStore.providers"
        :key="provider.id"
        :provider="provider"
        selectable
        :selected="onboardingLlm.defaultCli === provider.id"
        @select="selectProvider"
      />
    </div>

    <div class="flex flex-col gap-3.5 rounded-md border border-line bg-surface p-4">
      <span class="text-control font-semibold text-ink">Preferência para o time</span>

      <label v-if="providerOptions.length > 0" class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">Provedor preferido</span>
        <UiSelect
          v-model="onboardingLlm.defaultCli"
          data-testid="onboarding-default-provider"
          :options="providerOptions"
          label="Provedor preferido"
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
</template>
