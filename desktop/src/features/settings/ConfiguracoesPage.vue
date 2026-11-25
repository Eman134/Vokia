<script setup lang="ts">
import { computed, nextTick, onMounted, ref, useId, watch } from 'vue'
import { PAGE_LABELS } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSelect from '../../ui/UiSelect.vue'
import UiSwitch from '../../ui/UiSwitch.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import { HAND_MARK_CLASSES, INK_FOCUS_CLASSES, handReveal } from '../../ui/handDrawn'
import { trapFocus } from '../backlog/dialogFocus'
import UserAvatar from '../profile/UserAvatar.vue'
import UserProfileForm from '../profile/UserProfileForm.vue'
import { useProfileDraft } from '../profile/useProfileDraft'
import { useUserProfile } from '../profile/useUserProfile'
import { useActiveLlm, type FallbackChainItem } from '../providers/useActiveLlm'
import { useProviders } from '../providers/useProviders'
import type { ModelTier } from '../team/teamApi'
import SettingRow from './SettingRow.vue'
import SettingsSection from './SettingsSection.vue'
import { AGENTS_SECTION_TITLE, GENERAL_SECTION_TITLE, SETTINGS_SECTIONS } from './configuracoesSample'


const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col overflow-y-auto ${PAGE_INSET_CLASSES}`
const NAV_ITEM_CLASSES = `group relative block w-full cursor-pointer rounded-md px-3 py-2 text-left text-control ${INK_FOCUS_CLASSES}`
const NAV_ITEM_ACTIVE_CLASSES = 'font-semibold text-ink'
const NAV_ITEM_IDLE_CLASSES = 'text-muted hover:bg-surface-2 hover:text-ink'
const NAV_ITEM_MARK_CLASSES = `${HAND_MARK_CLASSES} bottom-0.5 left-3 w-[calc(100%-1.5rem)]`
const NAV_CLASSES = 'flex w-full shrink-0 flex-col gap-0.5 lg:w-[210px]'
const PROFILE_TITLE = 'Seu perfil'
const PROFILE_AVATAR_SIZE = 40
const PROFILE_CLASSES = 'flex max-w-[860px] flex-col gap-4 rounded-md border border-line bg-surface p-4'
// Sem w-full: a coluna divide a linha com a navegação em vez de ocupar 100% e encostar na borda direita.
const COLUMN_CLASSES = 'flex min-w-0 max-w-[860px] flex-1 basis-0 flex-col gap-4'

const TIER_OPTIONS: readonly { label: string; value: ModelTier }[] = [
  { label: 'Básico', value: 'basic' },
  { label: 'Intermediário', value: 'intermediate' },
  { label: 'Avançado', value: 'advanced' },
]

const profile = useUserProfile()
const draft = useProfileDraft()
const providersStore = useProviders()
const activeLlm = useActiveLlm()
const navigation = useNavigation()
const editing = ref(false)

const initialSection = navigation.takePendingSettingsSection()
const activeSection = ref(
  initialSection && SETTINGS_SECTIONS.some((section) => section.title === initialSection)
    ? initialSection
    : GENERAL_SECTION_TITLE,
)
watch(
  () => navigation.pendingSettingsSection,
  (nextSection) => {
    if (nextSection === null) return
    const section = navigation.takePendingSettingsSection()
    if (section && SETTINGS_SECTIONS.some((entry) => entry.title === section)) {
      activeSection.value = section
    }
  },
)
const fallbackEnabled = ref(true)
const fallbackCooldown = ref(300)
const fallbackChain = ref<FallbackChainItem[]>([])
const newFallbackCli = ref('')
const newFallbackTier = ref<ModelTier>('intermediate')
const savingFallback = ref(false)
const fallbackSavedMsg = ref<string | null>(null)

// Modal de confirmação para remover item da cadeia de fallback
const removeModalOpen = ref(false)
const removingFallbackIndex = ref<number | null>(null)
const removeDialog = ref<HTMLElement>()
const removeTitleId = useId()

const installedCount = computed(() => providersStore.providers.filter((p) => p.installed).length)
const providerSelectOptions = computed(() =>
  providersStore.providers.map((p) => ({ label: p.label, value: p.id })),
)

onMounted(async () => {
  await Promise.all([
    providersStore.load().catch(() => {}),
    activeLlm.load().catch(() => {}),
  ])
  fallbackEnabled.value = activeLlm.fallback.enabled
  fallbackCooldown.value = activeLlm.fallback.cooldownSecs
  fallbackChain.value = [...activeLlm.fallback.chain]
  if (!newFallbackCli.value && providersStore.providers.length > 0) {
    newFallbackCli.value = providersStore.providers[0].id
  }
})

function edit(): void {
  draft.begin()
  editing.value = true
}

async function save(): Promise<void> {
  if (await draft.submit()) editing.value = false
}

function addFallbackItem(): void {
  if (!newFallbackCli.value) return
  fallbackChain.value = [
    ...fallbackChain.value,
    { cli: newFallbackCli.value, modelTier: newFallbackTier.value },
  ]
}

function moveFallback(index: number, delta: -1 | 1): void {
  const target = index + delta
  if (target < 0 || target >= fallbackChain.value.length) return
  const next = [...fallbackChain.value]
  const [item] = next.splice(index, 1)
  next.splice(target, 0, item)
  fallbackChain.value = next
}

async function askRemoveFallback(index: number): Promise<void> {
  removingFallbackIndex.value = index
  removeModalOpen.value = true
  await nextTick()
  removeDialog.value?.focus()
}

function cancelRemoveFallback(): void {
  removeModalOpen.value = false
  removingFallbackIndex.value = null
}

function confirmRemoveFallback(): void {
  if (removingFallbackIndex.value === null) return
  const index = removingFallbackIndex.value
  fallbackChain.value = fallbackChain.value.filter((_, idx) => idx !== index)
  cancelRemoveFallback()
}

function providerLabel(cli: string): string {
  return providersStore.providers.find((p) => p.id === cli)?.label ?? cli
}

async function saveFallbackConfig(): Promise<void> {
  savingFallback.value = true
  fallbackSavedMsg.value = null
  try {
    await activeLlm.saveFallback({
      enabled: fallbackEnabled.value,
      cooldownSecs: Math.max(0, Number(fallbackCooldown.value) || 0),
      chain: fallbackChain.value,
    })
    fallbackSavedMsg.value = 'Configuração de fallback salva.'
  } finally {
    savingFallback.value = false
  }
}
</script>

<template>
  <div :class="PAGE_CLASSES">
    <UiPageHeader :title="PAGE_LABELS.config" />

    <div class="mt-5 flex flex-col items-start gap-6 lg:flex-row">
      <nav aria-label="Seções" :class="NAV_CLASSES">
        <button
          v-for="section in SETTINGS_SECTIONS"
          :key="section.title"
          type="button"
          :aria-current="section.title === activeSection ? 'page' : undefined"
          :class="[NAV_ITEM_CLASSES, section.title === activeSection ? NAV_ITEM_ACTIVE_CLASSES : NAV_ITEM_IDLE_CLASSES]"
          @click="activeSection = section.title"
        >
          {{ section.title }}
          <UiHandLine :class="[NAV_ITEM_MARK_CLASSES, handReveal(section.title === activeSection)]" />
        </button>

        <div class="mt-3.5 flex w-full flex-col gap-2.5 rounded-md bg-surface-2 p-3 text-small">
          <span class="flex items-center gap-1.5 font-semibold text-ink">
            <span
              class="size-2 rounded-full"
              :class="installedCount > 0 ? 'bg-ok' : 'bg-warn'"
              aria-hidden="true"
            />
            Provedores ({{ installedCount }}/{{ providersStore.providers.length }})
          </span>
          <ul v-if="providersStore.providers.length > 0" class="flex flex-col gap-1">
            <li v-for="provider in providersStore.providers" :key="provider.id">
              <button
                type="button"
                class="flex w-full cursor-pointer items-center justify-between gap-2 rounded-md px-1.5 py-1 text-left hover:bg-hover"
                @click="providersStore.openSetup(provider.id)"
              >
                <span class="flex min-w-0 items-center gap-1.5">
                  <span
                    class="size-1.5 shrink-0 rounded-full"
                    :class="provider.installed ? 'bg-ok' : 'bg-faint'"
                    aria-hidden="true"
                  />
                  <span class="truncate text-ink">{{ provider.label }}</span>
                </span>
                <span class="shrink-0 text-tiny text-muted">Editar</span>
              </button>
            </li>
          </ul>
          <span v-else class="text-muted">Nenhum provedor carregado</span>
          <UiButton
            data-testid="open-setup-llms"
            variant="quiet"
            size="sm"
            class="w-full"
            @press="providersStore.openSetup()"
          >
            Configurar LLMs
          </UiButton>
        </div>
      </nav>

      <div :class="COLUMN_CLASSES">
        <section v-if="activeSection === GENERAL_SECTION_TITLE" :class="PROFILE_CLASSES" data-testid="profile-row" :aria-label="PROFILE_TITLE">
          <div class="flex items-center gap-3">
            <UserAvatar :size="PROFILE_AVATAR_SIZE" />
            <div class="flex min-w-0 flex-1 flex-col">
              <span class="text-label text-muted">{{ PROFILE_TITLE }}</span>
              <b class="truncate text-control text-ink" :title="profile.displayName">{{ profile.displayName }}</b>
            </div>
            <UiButton v-if="!editing" variant="quiet" size="lg" @press="edit">Editar</UiButton>
          </div>
          <template v-if="editing">
            <UserProfileForm />
            <div class="flex justify-end gap-2">
              <UiButton variant="quiet" size="lg" @press="editing = false">Cancelar</UiButton>
              <UiButton variant="primary" size="lg" :disabled="!draft.valid || draft.saving" @press="save">Salvar</UiButton>
            </div>
          </template>
        </section>
        <section
          v-if="activeSection === AGENTS_SECTION_TITLE"
          data-testid="fallback-chain-editor"
          aria-label="Fallback automático"
          class="flex flex-col gap-4 rounded-md border border-line bg-surface p-5"
        >
          <div class="flex items-center justify-between gap-3">
            <div>
              <h3 class="text-control font-semibold text-ink">Fallback automático</h3>
              <p class="text-caption text-muted">
                Alterna temporariamente quando o provedor atingir limite ou recusar o modelo.
              </p>
            </div>
            <UiSwitch v-model="fallbackEnabled" label="Ativar fallback automático" />
          </div>

          <label class="flex items-center gap-3 text-caption text-ink">
            <span class="text-label font-medium text-muted">Cooldown (s)</span>
            <input
              v-model.number="fallbackCooldown"
              type="number"
              min="0"
              class="h-8 w-24 rounded-md border border-line bg-surface px-2.5 text-control text-ink" :class="INK_FOCUS_CLASSES"
            />
          </label>

          <ul v-if="fallbackChain.length > 0" class="flex flex-col divide-y divide-line rounded-md border border-line">
            <li
              v-for="(item, index) in fallbackChain"
              :key="`${item.cli}-${index}`"
              class="flex items-center justify-between gap-2 px-3 py-2 text-caption"
            >
              <span class="text-ink">
                <span class="mr-1.5 font-mono text-muted">{{ index + 1 }}.</span>
                {{ providerLabel(item.cli) }}
                <span class="text-muted">
                  · {{ TIER_OPTIONS.find((t) => t.value === item.modelTier)?.label ?? item.model ?? 'Padrão' }}
                </span>
              </span>
              <div class="flex items-center gap-1">
                <UiButton variant="quiet" size="sm" :disabled="index === 0" @press="moveFallback(index, -1)">
                  Subir
                </UiButton>
                <UiButton
                  variant="quiet"
                  size="sm"
                  :disabled="index === fallbackChain.length - 1"
                  @press="moveFallback(index, 1)"
                >
                  Descer
                </UiButton>
                <UiButton variant="danger" size="sm" @press="askRemoveFallback(index)">
                  Remover
                </UiButton>
              </div>
            </li>
          </ul>
          <p v-else class="text-caption text-muted">Nenhum provedor na cadeia global.</p>

          <div data-testid="fallback-add-row" class="flex flex-wrap items-center gap-2 pt-1">
            <UiSelect v-model="newFallbackCli" :options="providerSelectOptions" label="Provedor de fallback" />
            <UiSelect v-model="newFallbackTier" :options="TIER_OPTIONS" label="Nível de fallback" />
            <UiButton variant="quiet" size="sm" :disabled="!newFallbackCli" @press="addFallbackItem()">
              Adicionar
            </UiButton>
            <UiButton
              variant="primary"
              size="sm"
              class="ml-auto"
              :disabled="savingFallback"
              @press="saveFallbackConfig()"
            >
              {{ savingFallback ? 'Salvando...' : 'Salvar fallback' }}
            </UiButton>
          </div>
          <p v-if="fallbackSavedMsg" class="text-small text-ok">{{ fallbackSavedMsg }}</p>
        </section>

        <template v-for="section in SETTINGS_SECTIONS" :key="section.title">
          <SettingsSection
            v-if="section.title === activeSection"
            :title="section.title"
            :description="section.description"
            :action="section.action"
          >
            <SettingRow v-for="(item, index) in section.items" :key="index" :item="item" />
          </SettingsSection>
        </template>
      </div>
    </div>

    <!-- Modal de confirmação para remover item do fallback -->
    <div
      v-if="removeModalOpen && removingFallbackIndex !== null"
      class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4"
      @click.self="cancelRemoveFallback()"
    >
      <section
        ref="removeDialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="removeTitleId"
        tabindex="-1"
        class="relative flex w-100 max-w-full flex-col gap-4 rounded-md border border-line bg-surface p-5 shadow-paper"
        @keydown.esc.stop="cancelRemoveFallback()"
        @keydown.tab="trapFocus($event, removeDialog)"
      >
        <UiCloseButton class="absolute right-3 top-3 z-10" @press="cancelRemoveFallback()" />
        <h3 :id="removeTitleId" class="text-control font-bold text-ink pr-8">
          Remover {{ providerLabel(fallbackChain[removingFallbackIndex]?.cli ?? '') }} do fallback?
        </h3>
        <p class="text-caption text-muted">
          Este item será retirado da cadeia global de fallback automático.
        </p>
        <div class="flex justify-end gap-2">
          <UiButton variant="quiet" size="lg" @press="cancelRemoveFallback()">Cancelar</UiButton>
          <UiButton variant="danger" size="lg" @press="confirmRemoveFallback()">Remover</UiButton>
        </div>
      </section>
    </div>
  </div>
</template>
