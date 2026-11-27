<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { PAGE_LABELS } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import UiButton from '../../ui/UiButton.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import { HAND_MARK_CLASSES, INK_FOCUS_CLASSES, handReveal } from '../../ui/handDrawn'
import UserAvatar from '../profile/UserAvatar.vue'
import UserProfileForm from '../profile/UserProfileForm.vue'
import { useProfileDraft } from '../profile/useProfileDraft'
import { useUserProfile } from '../profile/useUserProfile'
import FallbackSettingsPanel from '../providers/FallbackSettingsPanel.vue'
import ProviderConnectionCard from '../providers/ProviderConnectionCard.vue'
import { useProviders } from '../providers/useProviders'
import SettingRow from './SettingRow.vue'
import SettingsSection from './SettingsSection.vue'
import WebSearchSettingsSection from './WebSearchSettingsSection.vue'
import {
  AGENTS_SECTION_TITLE,
  FALLBACK_SECTION_TITLE,
  GENERAL_SECTION_TITLE,
  SEARCH_SECTION_TITLE,
  SETTINGS_NAV_GROUPS,
  SETTINGS_SECTIONS,
} from './configuracoesSample'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col overflow-y-auto ${PAGE_INSET_CLASSES}`
const NAV_ITEM_CLASSES = `group relative block w-full cursor-pointer rounded-md px-3 py-2 text-left text-control ${INK_FOCUS_CLASSES}`
const NAV_ITEM_ACTIVE_CLASSES = 'font-semibold text-ink'
const NAV_ITEM_IDLE_CLASSES = 'text-muted hover:bg-surface-2 hover:text-ink'
const NAV_ITEM_MARK_CLASSES = `${HAND_MARK_CLASSES} bottom-0.5 left-3 w-[calc(100%-1.5rem)]`
const NAV_CLASSES = 'flex w-full shrink-0 flex-col gap-3 lg:w-[220px]'
const PROFILE_TITLE = 'Seu perfil'
const PROFILE_AVATAR_SIZE = 40
const PROFILE_CLASSES = 'flex max-w-[860px] flex-col gap-4 rounded-md border border-line bg-surface p-4'
const COLUMN_CLASSES = 'flex min-w-0 max-w-[860px] flex-1 basis-0 flex-col gap-4'

const ALL_NAV_SECTIONS = new Set<string>(SETTINGS_NAV_GROUPS.flatMap((group) => group.sections))

const profile = useUserProfile()
const draft = useProfileDraft()
const providersStore = useProviders()
const navigation = useNavigation()
const editing = ref(false)

const initialSection = navigation.takePendingSettingsSection()
const activeSection = ref(
  initialSection && ALL_NAV_SECTIONS.has(initialSection) ? initialSection : GENERAL_SECTION_TITLE,
)
watch(
  () => navigation.pendingSettingsSection,
  (nextSection) => {
    if (nextSection === null) return
    const section = navigation.takePendingSettingsSection()
    if (section && ALL_NAV_SECTIONS.has(section)) {
      activeSection.value = section
    }
  },
)

const installedCount = computed(() => providersStore.providers.filter((p) => p.installed).length)

onMounted(async () => {
  await providersStore.load().catch(() => {})
})

function edit(): void {
  draft.begin()
  editing.value = true
}

async function save(): Promise<void> {
  if (await draft.submit()) editing.value = false
}
</script>

<template>
  <div :class="PAGE_CLASSES">
    <UiPageHeader :title="PAGE_LABELS.config" />

    <div class="mt-5 flex flex-col items-start gap-6 lg:flex-row">
      <nav aria-label="Seções" :class="NAV_CLASSES">
        <div v-for="group in SETTINGS_NAV_GROUPS" :key="group.label" class="flex flex-col gap-0.5">
          <span class="px-3 pb-1 text-tiny font-semibold uppercase tracking-wide text-faint">
            {{ group.label }}
          </span>
          <button
            v-for="title in group.sections"
            :key="title"
            type="button"
            :aria-current="title === activeSection ? 'page' : undefined"
            :class="[NAV_ITEM_CLASSES, title === activeSection ? NAV_ITEM_ACTIVE_CLASSES : NAV_ITEM_IDLE_CLASSES]"
            @click="activeSection = title"
          >
            {{ title }}
            <UiHandLine :class="[NAV_ITEM_MARK_CLASSES, handReveal(title === activeSection)]" />
          </button>
        </div>

        <div class="mt-1 flex w-full flex-col gap-2.5 rounded-md bg-surface-2 p-3 text-small">
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
          data-testid="settings-providers-panel"
          aria-label="Provedores conectados"
          class="flex flex-col gap-3 rounded-md border border-line bg-surface p-5"
        >
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div>
              <h3 class="text-control font-semibold text-ink">Provedores e conexões</h3>
              <p class="text-caption text-muted">
                Conecte, ative/desative, gerencie variáveis de ambiente ou desconecte cada provedor.
              </p>
            </div>
            <div class="flex items-center gap-2">
              <UiButton
                variant="quiet"
                size="sm"
                @press="providersStore.checkStatus()"
              >
                Verificar todos
              </UiButton>
              <UiButton
                variant="quiet"
                size="sm"
                @press="providersStore.openAddProvider()"
              >
                + Adicionar provedor
              </UiButton>
            </div>
          </div>
          <div class="flex flex-col gap-3">
            <ProviderConnectionCard
              v-for="provider in providersStore.providers"
              :key="provider.id"
              :provider="provider"
              show-settings-controls
            />
          </div>
        </section>

        <FallbackSettingsPanel v-if="activeSection === FALLBACK_SECTION_TITLE" />

        <WebSearchSettingsSection v-if="activeSection === SEARCH_SECTION_TITLE" />

        <template v-for="section in SETTINGS_SECTIONS" :key="section.title">
          <SettingsSection
            v-if="section.title === activeSection && section.title !== SEARCH_SECTION_TITLE && section.title !== AGENTS_SECTION_TITLE"
            :title="section.title"
            :description="section.description"
            :action="section.action"
          >
            <SettingRow v-for="(item, index) in section.items" :key="index" :item="item" />
          </SettingsSection>
        </template>
      </div>
    </div>
  </div>
</template>
