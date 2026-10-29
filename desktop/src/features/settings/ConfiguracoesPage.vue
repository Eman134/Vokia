<script setup lang="ts">
import { PAGE_LABELS, SAMPLE_DATA_LABEL } from '../../shell/railNav'
import SettingRow from './SettingRow.vue'
import SettingsSection from './SettingsSection.vue'
import { SETTINGS_SECTIONS } from './configuracoesSample'

const CONTEXT_LABEL = 'Workspace'
const PAGE_SUBTITLE = 'Valem para todos os projetos. Regras de um projeto ou de um agente ficam nas páginas deles e têm prioridade.'
const SEARCH_LABEL = 'Buscar configuração'
const ACTIVE_SECTION_TITLE = 'Geral'
const CONNECTION_STATUS = 'Tudo conectado'
const CONNECTION_DETAIL = 'Claude Code, Codex, GitHub CLI e Git'

const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col overflow-y-auto pt-7 pr-8 pb-37.5 pl-2'
const SEARCH_CLASSES =
  'ml-auto h-10 w-65 rounded-card border border-line bg-surface px-3.5 text-control text-ink placeholder:text-faint disabled:opacity-40'
const NAV_CLASSES = 'flex w-[210px] shrink-0 flex-col gap-0.5'
const NAV_ITEM_CLASSES =
  'block w-full cursor-default rounded-control px-3 py-2 text-left text-control disabled:opacity-40'
const NAV_ITEM_ACTIVE_CLASSES = 'bg-accent-soft font-semibold text-accent'
const NAV_ITEM_IDLE_CLASSES = 'text-muted'
const COLUMN_CLASSES = 'flex min-w-0 max-w-[860px] flex-1 flex-col gap-4'
</script>

<template>
  <div :class="PAGE_CLASSES">
    <header class="flex flex-wrap items-end gap-4">
      <div>
        <p class="text-caption text-muted">{{ CONTEXT_LABEL }} · {{ SAMPLE_DATA_LABEL }}</p>
        <h1 class="mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink">{{ PAGE_LABELS.config }}</h1>
        <p class="mt-1 max-w-[70ch] text-body text-muted">{{ PAGE_SUBTITLE }}</p>
      </div>
      <input type="search" :aria-label="SEARCH_LABEL" :placeholder="SEARCH_LABEL" :class="SEARCH_CLASSES" disabled aria-disabled="true" />
    </header>

    <div class="mt-5 flex items-start gap-6">
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
