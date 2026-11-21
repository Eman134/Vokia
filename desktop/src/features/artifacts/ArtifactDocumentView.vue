<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { tagLabel } from '../backlog/tags'
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

const TAG_CLASSES = 'rounded-full bg-surface-2 px-2 py-0.5 text-meta text-muted'
const DATE_LOCALE = 'pt-BR'

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
  <article class="flex min-w-0 flex-col gap-6" :aria-label="`Artefato ${document.title}`">
    <header class="flex min-w-0 flex-col gap-2">
      <p class="text-caption text-muted">Artefatos › {{ kindLabel }} · <span class="font-mono">{{ document.id }}</span></p>
      <div class="flex flex-wrap items-center gap-2.5">
        <h3 :class="['min-w-0 flex-1 text-lead text-ink', WRAP_CLASSES]">{{ document.title }}</h3>
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
        <span v-for="tag in document.tags" :key="tag" :class="TAG_CLASSES">{{ tagLabel(tag) }}</span>
      </div>
    </header>

    <UiNotice v-if="artifacts.error" :text="artifacts.error" @close="artifacts.clearFeedback()" />

    <section aria-label="Conteúdo" class="min-w-0">
      <MarkdownText v-if="document.body.trim()" :key="renderKey" :text="document.body" tasks @toggle="toggle" />
      <p v-else class="text-small text-muted">{{ DOCUMENT_EMPTY_BODY }}</p>
    </section>
  </article>
</template>
