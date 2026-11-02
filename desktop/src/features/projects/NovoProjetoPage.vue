<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from 'vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { useFlow } from '../team/useFlow'
import { SAMPLE_DATA_LABEL } from '../../shell/railNav'
import PrimeiroPedidoStep from './PrimeiroPedidoStep.vue'
import ProjetoOrigemStep from './ProjetoOrigemStep.vue'
import TimeStep from './TimeStep.vue'
import ProjectIcon from './ProjectIcon.vue'
import {
  CLI_NOTE,
  CREATES_HEADING,
  CREATES_ITEMS,
  CREATES_NOTE,
  MAP_PREVIEW_HEADING,
  MAP_PREVIEW_NOTE,
  NEW_PROJECT_CONTEXT,
  PREVIEW_HEADING,
  SAMPLE_TEAM_PREVIEW,
  SHARED_HEADING,
  SHARED_NOTE,
  STEP_SUBTITLES,
  STEP_TITLES,
  SUMMARY_HEADING,
  TEAM_NEXT_HEADING,
} from './projetosSample'
import { useNovoProjeto, type WizardStep } from './useNovoProjeto'

const PREVIEW_PORTRAIT_SIZE = 28
const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto pt-7 pr-8 pb-[150px] pl-2'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const SUBTITLE_CLASSES = 'mt-1 max-w-[64ch] text-body text-muted'
const COLUMNS_CLASSES = 'flex flex-wrap items-start gap-5.5'
const FORM_CLASSES = 'flex min-w-0 basis-[560px] flex-1 flex-col gap-5.5 rounded-[28px] bg-surface p-7'
const PREVIEW_CLASSES = 'flex min-w-0 basis-[360px] flex-col gap-4 rounded-[28px] border border-line p-6'
const HEADING_CLASSES = 'text-label font-medium text-muted'

const wizard = useNovoProjeto()
const flow = useFlow()

flow.start('novo-projeto', wizard.step)
watch(
  () => wizard.step,
  (value) => flow.goTo(value),
)
watch(
  () => flow.step,
  (value) => wizard.goBackTo(value as WizardStep),
)
onBeforeUnmount(() => flow.finish())

const index = computed(() => wizard.step - 1)
const previewName = computed(() => wizard.name || 'novo-projeto')
const previewPath = computed(() =>
  wizard.originKind === 'folder'
    ? wizard.path || 'Pasta ainda não escolhida'
    : `${wizard.repo || 'dono/repo'} → ${wizard.parentDir || 'pasta de destino'}`,
)
const previewOrigin = computed(() => (wizard.originKind === 'folder' ? 'Pasta' : 'GitHub'))
</script>

<template>
  <section :class="PAGE_CLASSES">
    <header>
      <p class="text-caption text-muted" data-testid="sample-label">
        {{ NEW_PROJECT_CONTEXT }} · {{ SAMPLE_DATA_LABEL }}
      </p>
      <h1 :class="TITLE_CLASSES">{{ STEP_TITLES[index] }}</h1>
      <p :class="SUBTITLE_CLASSES">{{ STEP_SUBTITLES[index] }}</p>
    </header>

    <div :class="COLUMNS_CLASSES">
      <div :class="FORM_CLASSES">
        <ProjetoOrigemStep v-if="wizard.step === 1" />
        <TimeStep v-else-if="wizard.step === 2" />
        <PrimeiroPedidoStep v-else />
      </div>

      <aside :class="PREVIEW_CLASSES">
        <template v-if="wizard.step === 1">
          <h2 :class="HEADING_CLASSES">{{ PREVIEW_HEADING }}</h2>
          <div class="flex items-center gap-3">
            <span class="grid size-10 place-items-center rounded-control bg-accent-soft text-accent">
              <ProjectIcon :icon="wizard.symbol" class="size-5" data-testid="preview-icon" />
            </span>
            <div class="min-w-0">
              <b class="block truncate text-ink">{{ previewName }}</b>
              <span class="block truncate font-mono text-small text-muted">{{ previewPath }}</span>
            </div>
          </div>
          <p class="text-caption text-muted">{{ previewOrigin }}</p>
          <h2 :class="HEADING_CLASSES">{{ TEAM_NEXT_HEADING }}</h2>
          <div class="flex">
            <AgentPortrait
              v-for="(who, position) in SAMPLE_TEAM_PREVIEW"
              :key="who"
              :who="who"
              :size="PREVIEW_PORTRAIT_SIZE"
              :class="position > 0 && '-ml-2'"
            />
          </div>
          <h2 :class="HEADING_CLASSES">{{ CREATES_HEADING }}</h2>
          <ul class="flex flex-col gap-1.5 text-caption text-muted">
            <li v-for="item in CREATES_ITEMS" :key="item.name">
              <span class="font-mono text-ink">{{ item.name }}</span> {{ item.text }}
            </li>
          </ul>
          <p class="text-caption text-muted">{{ CREATES_NOTE }}</p>
          <p class="text-caption text-muted">{{ CLI_NOTE }}</p>
        </template>
        <template v-else-if="wizard.step === 2">
          <h2 :class="HEADING_CLASSES">{{ MAP_PREVIEW_HEADING }}</h2>
          <p class="text-caption text-ink">{{ previewName }}</p>
          <p class="text-caption text-muted">{{ MAP_PREVIEW_NOTE }}</p>
          <h2 :class="HEADING_CLASSES">{{ SHARED_HEADING }}</h2>
          <p class="text-caption text-muted">{{ SHARED_NOTE }}</p>
        </template>
        <template v-else>
          <h2 :class="HEADING_CLASSES">{{ SUMMARY_HEADING }}</h2>
          <dl class="flex flex-col gap-1.5 text-caption">
            <div class="flex gap-2"><dt class="w-24 text-muted">Origem</dt><dd class="min-w-0 truncate text-ink">{{ previewPath }}</dd></div>
            <div class="flex gap-2"><dt class="w-24 text-muted">Tipo</dt><dd class="text-ink">{{ previewOrigin }}</dd></div>
            <div class="flex gap-2"><dt class="w-24 text-muted">Nome</dt><dd class="text-ink">{{ previewName }}</dd></div>
          </dl>
        </template>
      </aside>
    </div>
  </section>
</template>
