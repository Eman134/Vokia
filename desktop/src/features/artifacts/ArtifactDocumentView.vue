<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiMarker from '../../ui/UiMarker.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { DEFAULT_JITTER, sketchLine } from '../../ui/sketch/sketchPath'
import { tagLabel, tagMarkerTone } from '../backlog/tags'
import { agentName } from '../team/agents'
import MarkdownText from '../team/MarkdownText.vue'
import type { ArtifactDocument } from './artifactApi'
import { WRAP_CLASSES } from './artifactCardClasses'
import {
  DOCUMENT_ANONYMOUS_AUTHOR,
  DOCUMENT_DELETE_CONFIRM_LABEL,
  DOCUMENT_DELETE_LABEL,
  DOCUMENT_DELETE_MESSAGE,
  DOCUMENT_EDIT_LABEL,
  DOCUMENT_EMPTY_BODY,
} from './artifactsCopy'
import { useArtifacts } from './useArtifacts'

const DATE_LOCALE = 'pt-BR'
const MARGIN_HEIGHT = 400
const MARGIN_SEED = 4
// A margem de caderno é um traço vertical que estica com a folha; o desenho cabe numa altura fixa.
const MARGIN_PATH = sketchLine(4, 0, 4, MARGIN_HEIGHT, MARGIN_SEED, DEFAULT_JITTER)
const SHEET_CLASSES =
  'relative flex min-w-0 max-w-[68ch] flex-col gap-6 rounded-sm border border-line bg-surface py-6 pr-6 pl-14 shadow-paper'
const MARGIN_CLASSES = 'pointer-events-none absolute inset-y-0 left-9 h-full w-2 overflow-visible text-danger opacity-40'
const TITLE_CLASSES = `text-card-title text-ink ${WRAP_CLASSES}`

const props = defineProps<{ document: ArtifactDocument; kindLabel: string }>()
const emit = defineEmits<{ edit: []; removed: [] }>()

const artifacts = useArtifacts()
// Uma leitura recusada deixa o checkbox marcado no navegador; trocar a chave refaz o HTML a partir do documento.
const renderKey = ref(0)

const author = computed(() => (props.document.author ? agentName(props.document.author) : DOCUMENT_ANONYMOUS_AUTHOR))
const created = computed(() => new Date(props.document.createdAt).toLocaleString(DATE_LOCALE))
const updated = computed(() => new Date(props.document.updatedAt).toLocaleString(DATE_LOCALE))

async function toggle(index: number, done: boolean): Promise<void> {
  await artifacts.toggleCheck(props.document.id, index, done)
  renderKey.value += 1
}

async function remove(): Promise<void> {
  if (await artifacts.remove(props.document.id)) emit('removed')
}
</script>

<template>
  <article :class="SHEET_CLASSES" :aria-label="`Artefato ${document.title}`">
    <svg :class="MARGIN_CLASSES" :viewBox="`0 0 8 ${MARGIN_HEIGHT}`" preserveAspectRatio="none" fill="none" stroke="currentColor" aria-hidden="true">
      <path :d="MARGIN_PATH" stroke-width="1.2" stroke-linecap="round" vector-effect="non-scaling-stroke" />
    </svg>
    <header class="flex min-w-0 flex-col gap-2">
      <p class="text-caption text-muted">Artefatos › {{ kindLabel }} · <span class="font-mono">{{ document.id }}</span></p>
      <div class="flex flex-wrap items-center gap-2.5">
        <div class="min-w-0 flex-1">
          <h3 :class="TITLE_CLASSES">{{ document.title }}</h3>
          <UiHandLine class="h-1.5 w-full max-w-40 text-graphite" />
        </div>
        <div class="flex shrink-0 flex-wrap items-center gap-2">
          <UiButton variant="quiet" size="sm" :disabled="artifacts.saving" data-testid="artifact-edit" @press="emit('edit')">
            {{ DOCUMENT_EDIT_LABEL }}
          </UiButton>
          <UiConfirm
            :label="DOCUMENT_DELETE_LABEL"
            :message="DOCUMENT_DELETE_MESSAGE"
            :confirm-label="DOCUMENT_DELETE_CONFIRM_LABEL"
            :busy="artifacts.saving"
            @confirm="remove"
          />
        </div>
      </div>
      <p class="text-small text-muted">{{ author }} · criado em {{ created }} · atualizado em {{ updated }}</p>
      <p v-if="document.summary" :class="['whitespace-pre-wrap text-body text-ink', WRAP_CLASSES]">{{ document.summary }}</p>
      <div v-if="document.tags.length" class="flex flex-wrap gap-1.5">
        <UiMarker v-for="tag in document.tags" :key="tag" :tone="tagMarkerTone(tag)" class="text-meta">{{ tagLabel(tag) }}</UiMarker>
      </div>
    </header>

    <UiNotice v-if="artifacts.error" :text="artifacts.error" @close="artifacts.clearFeedback()" />

    <section aria-label="Conteúdo" class="min-w-0">
      <MarkdownText v-if="document.body.trim()" :key="renderKey" :text="document.body" tasks @toggle="toggle" />
      <p v-else class="text-small text-muted">{{ DOCUMENT_EMPTY_BODY }}</p>
    </section>
  </article>
</template>
