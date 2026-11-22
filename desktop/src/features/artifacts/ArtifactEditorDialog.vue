<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { trapFocus } from '../backlog/dialogFocus'
import { normalizeTag } from '../backlog/tags'
import { useProjects } from '../projects/useProjects'
import MarkdownText from '../team/MarkdownText.vue'
import type { ArtifactDocument, ArtifactKindId, ArtifactKindInfo } from './artifactApi'
import {
  EDITOR_BODY_LABEL,
  EDITOR_BODY_MODE_LABEL,
  EDITOR_CANCEL_LABEL,
  EDITOR_CONFLICT_MESSAGE,
  EDITOR_CREATE_LABEL,
  EDITOR_CREATE_TITLE,
  EDITOR_DISCARD_LABEL,
  EDITOR_DISCARD_MESSAGE,
  EDITOR_EDIT_TITLE,
  EDITOR_KEEP_LABEL,
  EDITOR_KIND_LABEL,
  EDITOR_PREVIEW_EMPTY,
  EDITOR_PREVIEW_LABEL,
  EDITOR_RELOAD_LABEL,
  EDITOR_SAVE_LABEL,
  EDITOR_SAVING_LABEL,
  EDITOR_SUMMARY_LABEL,
  EDITOR_SUMMARY_PLACEHOLDER,
  EDITOR_TAGS_LABEL,
  EDITOR_TAGS_PLACEHOLDER,
  EDITOR_TITLE_LABEL,
  EDITOR_TITLE_MAX_CHARS,
  EDITOR_TITLE_PLACEHOLDER,
  EDITOR_WRITE_LABEL,
} from './artifactsCopy'
import { useArtifacts } from './useArtifacts'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiCloseButton from '../../ui/UiCloseButton.vue'

const OVERLAY_CLASSES = 'fixed inset-0 z-50 flex items-center justify-center bg-paper/70 p-4'
const FRAME_CLASSES = 'relative border border-line w-160 max-w-full rounded-md bg-surface shadow-paper'
// O backdrop tem p-4 (2rem no total): o diálogo rola por dentro em vez de passar da janela.
const DIALOG_CLASSES = 'flex max-h-[calc(100dvh-2rem)] flex-col gap-3 overflow-y-auto p-5'
const WRITE_TAB = 'escrever'
const PREVIEW_TAB = 'preview'
const TAG_SEPARATOR = ','
const BODY_ROWS = 14
const FIRST_FIELD_SELECTOR = 'input'
const LABEL_CLASSES = 'text-label font-medium text-muted'
const CONTROL_CLASSES =
  `w-full min-w-0 rounded-md border border-line bg-surface px-3 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`

type BodyTab = typeof WRITE_TAB | typeof PREVIEW_TAB

const props = defineProps<{ document?: ArtifactDocument }>()
const emit = defineEmits<{ close: []; saved: [document: ArtifactDocument] }>()

const artifacts = useArtifacts()
const projects = useProjects()
const titleId = useId()
const titleFieldId = useId()
const summaryFieldId = useId()
const tagsFieldId = useId()
const bodyFieldId = useId()
const dialog = ref<HTMLElement>()
let previousFocus: HTMLElement | null = null

const editing = computed(() => props.document !== undefined)
const kindOptions = computed(() => artifacts.kinds.map((info) => ({ label: info.label, value: info.kind })))
const initialKind = props.document?.kind ?? artifacts.kinds[0]?.kind ?? 'documento'

function templateOf(kind: ArtifactKindId): string {
  return artifacts.kinds.find((info: ArtifactKindInfo) => info.kind === kind)?.template ?? ''
}

function tagsToText(tags: readonly string[]): string {
  return tags.join(`${TAG_SEPARATOR} `)
}

function textToTags(text: string): string[] {
  const tags = text.split(TAG_SEPARATOR).flatMap((raw) => normalizeTag(raw) ?? [])
  return [...new Set(tags)]
}

const kind = ref<ArtifactKindId>(initialKind)
const title = ref(props.document?.title ?? '')
const summary = ref(props.document?.summary ?? '')
const tagsText = ref(tagsToText(props.document?.tags ?? []))
const body = ref(props.document?.body ?? templateOf(initialKind))
const tab = ref<BodyTab>(WRITE_TAB)
const askingDiscard = ref(false)
// A base da gravação: com ela o backend recusa a edição se outra pessoa ou agente alterou o artefato antes.
const baseline = ref({ updatedAt: props.document?.updatedAt, snapshot: snapshotOf() })
let lastTemplate = props.document ? '' : templateOf(initialKind)

function snapshotOf(): string {
  return JSON.stringify([kind.value, title.value.trim(), summary.value, textToTags(tagsText.value), body.value])
}

const dirty = computed(() => snapshotOf() !== baseline.value.snapshot)
const canSubmit = computed(() => !artifacts.saving && title.value.trim() !== '')
const tabOptions = [
  { label: EDITOR_WRITE_LABEL, value: WRITE_TAB },
  { label: EDITOR_PREVIEW_LABEL, value: PREVIEW_TAB },
] as const

// Trocar de tipo com o corpo ainda igual ao template anterior troca o template; texto já editado fica.
watch(kind, (next) => {
  if (editing.value) return
  if (body.value === lastTemplate) body.value = templateOf(next)
  lastTemplate = templateOf(next)
})

watch(
  () => projects.selected?.id,
  () => emit('close'),
)

onMounted(async () => {
  artifacts.clearFeedback()
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  await nextTick()
  dialog.value?.querySelector<HTMLElement>(FIRST_FIELD_SELECTOR)?.focus()
})

onBeforeUnmount(() => {
  previousFocus?.focus()
  artifacts.clearFeedback()
})

function requestClose(): void {
  if (artifacts.saving) return
  if (dirty.value) {
    askingDiscard.value = true
    return
  }
  emit('close')
}

async function submit(): Promise<void> {
  if (!canSubmit.value) return
  const tags = textToTags(tagsText.value)
  const saved = props.document
    ? await artifacts.save(props.document.id, {
        title: title.value.trim(),
        summary: summary.value,
        body: body.value,
        tags,
        expectedUpdatedAt: baseline.value.updatedAt,
      })
    : await artifacts.create({
        kind: kind.value,
        title: title.value.trim(),
        summary: summary.value,
        body: body.value,
        tags,
      })
  if (saved) emit('saved', saved)
}

async function reload(): Promise<void> {
  const fresh = await artifacts.reloadCurrent()
  if (!fresh) return
  title.value = fresh.title
  summary.value = fresh.summary
  tagsText.value = tagsToText(fresh.tags)
  body.value = fresh.body
  baseline.value = { updatedAt: fresh.updatedAt, snapshot: snapshotOf() }
}
</script>

<template>
  <div :class="OVERLAY_CLASSES" @click.self="requestClose">
    <div :class="FRAME_CLASSES">
      <UiCloseButton class="absolute right-3 top-3 z-10" @press="requestClose" />
    <section
      ref="dialog"
      data-testid="artifact-editor"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      :class="DIALOG_CLASSES"
      @keydown.esc.stop="requestClose"
      @keydown.tab="trapFocus($event, dialog)"
      @keydown.enter.ctrl.exact.prevent="submit"
      @keydown.enter.meta.exact.prevent="submit"
    >
      <b :id="titleId" class="text-control text-ink">{{ editing ? EDITOR_EDIT_TITLE : EDITOR_CREATE_TITLE }}</b>

      <div v-if="!editing" class="flex flex-col gap-1.5">
        <span :class="LABEL_CLASSES">{{ EDITOR_KIND_LABEL }}</span>
        <UiSegmented v-model="kind" :options="kindOptions" :label="EDITOR_KIND_LABEL" wrap />
      </div>

      <label class="flex flex-col gap-1.5" :for="titleFieldId">
        <span class="flex items-baseline justify-between gap-3">
          <span :class="LABEL_CLASSES">{{ EDITOR_TITLE_LABEL }}</span>
          <span class="text-tiny text-faint">{{ title.length }}/{{ EDITOR_TITLE_MAX_CHARS }}</span>
        </span>
        <input
          :id="titleFieldId"
          v-model="title"
          data-testid="artifact-title"
          :maxlength="EDITOR_TITLE_MAX_CHARS"
          :placeholder="EDITOR_TITLE_PLACEHOLDER"
          :class="[CONTROL_CLASSES, 'h-9']"
        />
      </label>

      <label class="flex flex-col gap-1.5" :for="summaryFieldId">
        <span :class="LABEL_CLASSES">{{ EDITOR_SUMMARY_LABEL }}</span>
        <input
          :id="summaryFieldId"
          v-model="summary"
          data-testid="artifact-summary"
          :placeholder="EDITOR_SUMMARY_PLACEHOLDER"
          :class="[CONTROL_CLASSES, 'h-9']"
        />
      </label>

      <label class="flex flex-col gap-1.5" :for="tagsFieldId">
        <span :class="LABEL_CLASSES">{{ EDITOR_TAGS_LABEL }}</span>
        <input
          :id="tagsFieldId"
          v-model="tagsText"
          data-testid="artifact-tags"
          :placeholder="EDITOR_TAGS_PLACEHOLDER"
          :class="[CONTROL_CLASSES, 'h-9']"
        />
      </label>

      <div class="flex min-w-0 flex-col gap-1.5">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <label :class="LABEL_CLASSES" :for="bodyFieldId">{{ EDITOR_BODY_LABEL }}</label>
          <UiSegmented v-model="tab" :options="tabOptions" :label="EDITOR_BODY_MODE_LABEL" />
        </div>
        <textarea
          v-if="tab === WRITE_TAB"
          :id="bodyFieldId"
          v-model="body"
          data-testid="artifact-body"
          :rows="BODY_ROWS"
          spellcheck="false"
          class="py-2 font-mono text-small"
          :class="CONTROL_CLASSES"
        />
        <div v-else class="max-h-96 min-h-40 overflow-y-auto rounded-md border border-line bg-surface p-3" data-testid="artifact-preview">
          <MarkdownText v-if="body.trim()" :text="body" />
          <p v-else class="text-small text-muted">{{ EDITOR_PREVIEW_EMPTY }}</p>
        </div>
      </div>

      <div v-if="artifacts.conflict" class="flex min-w-0 flex-wrap items-center gap-3 rounded-md bg-surface-2 py-2 pl-3.5 pr-2 text-caption text-ink" role="alert">
        <p class="min-w-0 flex-1">{{ EDITOR_CONFLICT_MESSAGE }}</p>
        <UiButton variant="quiet" size="sm" data-testid="artifact-reload" @press="reload">{{ EDITOR_RELOAD_LABEL }}</UiButton>
      </div>
      <UiNotice v-else-if="artifacts.error" :text="artifacts.error" @close="artifacts.clearFeedback()" />

      <div v-if="askingDiscard" class="flex min-w-0 flex-wrap items-center gap-2" role="alert">
        <p class="min-w-0 flex-1 text-caption text-muted">{{ EDITOR_DISCARD_MESSAGE }}</p>
        <UiButton variant="quiet" size="sm" data-testid="artifact-keep" @press="askingDiscard = false">{{ EDITOR_KEEP_LABEL }}</UiButton>
        <UiButton variant="danger" size="sm" data-testid="artifact-discard" @press="emit('close')">{{ EDITOR_DISCARD_LABEL }}</UiButton>
      </div>
      <div v-else class="flex justify-end gap-2">
        <UiButton data-testid="artifact-cancel" variant="quiet" size="lg" :disabled="artifacts.saving" @press="requestClose">
          {{ EDITOR_CANCEL_LABEL }}
        </UiButton>
        <UiButton data-testid="artifact-submit" variant="primary" size="lg" :disabled="!canSubmit" @press="submit">
          {{ artifacts.saving ? EDITOR_SAVING_LABEL : editing ? EDITOR_SAVE_LABEL : EDITOR_CREATE_LABEL }}
        </UiButton>
      </div>
    </section>
    </div>
  </div>
</template>
