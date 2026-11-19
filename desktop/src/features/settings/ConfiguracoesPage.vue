<script setup lang="ts">
import { ref } from 'vue'
import { PAGE_LABELS } from '../../shell/railNav'
import UiButton from '../../ui/UiButton.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UserAvatar from '../profile/UserAvatar.vue'
import UserProfileForm from '../profile/UserProfileForm.vue'
import { useProfileDraft } from '../profile/useProfileDraft'
import { useUserProfile } from '../profile/useUserProfile'
import SettingRow from './SettingRow.vue'
import SettingsSection from './SettingsSection.vue'
import { SETTINGS_SECTIONS } from './configuracoesSample'

const SEARCH_LABEL = 'Buscar configuração'
const ACTIVE_SECTION_TITLE = 'Geral'
const CONNECTION_STATUS = 'Tudo conectado'
const CONNECTION_DETAIL = 'Claude Code, Codex, GitHub CLI e Git'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col overflow-y-auto ${PAGE_INSET_CLASSES}`
const SEARCH_CLASSES =
  'h-10 w-65 rounded-card border border-line bg-surface px-3.5 text-control text-ink placeholder:text-faint disabled:opacity-40'
const NAV_CLASSES = 'flex w-full shrink-0 flex-col gap-0.5 lg:w-[210px]'
const NAV_ITEM_CLASSES =
  'block w-full cursor-default rounded-control px-3 py-2 text-left text-control disabled:opacity-40'
const NAV_ITEM_ACTIVE_CLASSES = 'bg-accent-soft font-semibold text-accent'
const NAV_ITEM_IDLE_CLASSES = 'text-muted'
const PROFILE_TITLE = 'Seu perfil'
const PROFILE_AVATAR_SIZE = 40
const PROFILE_CLASSES = 'mt-5 flex max-w-[860px] flex-col gap-4 rounded-card border border-line bg-surface p-4'
const COLUMN_CLASSES = 'flex w-full min-w-0 max-w-[860px] flex-1 flex-col gap-4'

const profile = useUserProfile()
const draft = useProfileDraft()
const editing = ref(false)

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
    <UiPageHeader :title="PAGE_LABELS.config">
      <template #actions>
        <input type="search" :aria-label="SEARCH_LABEL" :placeholder="SEARCH_LABEL" :class="SEARCH_CLASSES" disabled aria-disabled="true" />
      </template>
    </UiPageHeader>

    <section :class="PROFILE_CLASSES" data-testid="profile-row" :aria-label="PROFILE_TITLE">
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

    <div class="mt-5 flex flex-col items-start gap-6 lg:flex-row">
      <nav aria-label="Seções" :class="NAV_CLASSES">
        <button
          v-for="section in SETTINGS_SECTIONS"
          :key="section.title"
          type="button"
          disabled
          :aria-current="section.title === ACTIVE_SECTION_TITLE ? 'page' : undefined"
          :class="[
            NAV_ITEM_CLASSES,
            section.title === ACTIVE_SECTION_TITLE ? NAV_ITEM_ACTIVE_CLASSES : NAV_ITEM_IDLE_CLASSES,
          ]"
        >
          {{ section.title }}
        </button>
        <div class="mt-3.5 flex flex-col gap-1 rounded-card bg-surface-2 p-3 text-small">
          <span class="flex items-center gap-1.5 font-semibold">
            <span class="size-2 rounded-full bg-ok" aria-hidden="true" />
            {{ CONNECTION_STATUS }}
          </span>
          <span class="text-muted">{{ CONNECTION_DETAIL }}</span>
        </div>
      </nav>

      <div :class="COLUMN_CLASSES">
        <SettingsSection
          v-for="section in SETTINGS_SECTIONS"
          :key="section.title"
          :title="section.title"
          :description="section.description"
          :action="section.action"
        >
          <SettingRow v-for="(item, index) in section.items" :key="index" :item="item" />
        </SettingsSection>
      </div>
    </div>
  </div>
</template>
