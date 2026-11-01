<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { storeToRefs } from 'pinia'
import {
  BACK_LABEL,
  CREATE_WITHOUT_REQUEST_LABEL,
  LINKED_REPO_LABEL,
  NEXT_HEADING,
  NEXT_STEPS,
  REQUEST_COMPOSER_HINT,
  REQUEST_CONTEXT,
  REQUEST_CONTEXT_HEADING,
  REQUEST_CREW_NOTE,
  REQUEST_FIELD_LABEL,
  REQUEST_ISSUES,
  REQUEST_ISSUES_HEADING,
  REQUEST_LEAD,
  REQUEST_LEAD_NOTE,
  REQUEST_SEND_LABEL,
  REQUEST_START_HEADING,
  REQUEST_START_OPTIONS,
  REQUEST_SUGGESTIONS,
  REQUEST_SUGGESTIONS_HEADING,
  REQUEST_TEXT,
  REQUEST_TO_HEADING,
} from './projetosSample'
import { useNovoProjeto } from './useNovoProjeto'

const HEADING_CLASSES = 'text-label font-medium text-muted'
const DISABLED_ITEM_CLASSES = 'rounded-control border border-line bg-surface px-3 py-2 text-left opacity-60'

const wizard = useNovoProjeto()
const { busy, error, linkedRepo, originKind } = storeToRefs(wizard)
</script>

<template>
  <div class="flex flex-col gap-5.5" aria-disabled="true">
    <h2 :class="HEADING_CLASSES">{{ REQUEST_TO_HEADING }}</h2>
    <p class="text-caption text-ink">
      <b>{{ REQUEST_LEAD }}</b>{{ REQUEST_LEAD_NOTE }} · {{ REQUEST_CREW_NOTE }}
    </p>

    <label class="flex flex-col gap-1.5">
      <span :class="HEADING_CLASSES">{{ REQUEST_FIELD_LABEL }}</span>
      <textarea
        :value="REQUEST_TEXT"
        disabled
        aria-disabled="true"
        rows="4"
        class="w-full rounded-control border border-line bg-surface p-3 text-control text-ink opacity-60"
      />
      <span class="text-small text-faint">{{ REQUEST_COMPOSER_HINT }}</span>
    </label>

    <h2 :class="HEADING_CLASSES">{{ REQUEST_SUGGESTIONS_HEADING }}</h2>
    <div class="flex flex-wrap gap-2">
      <button v-for="label in REQUEST_SUGGESTIONS" :key="label" type="button" disabled :class="[DISABLED_ITEM_CLASSES, 'rounded-full']">
        {{ label }}
      </button>
    </div>

    <h2 :class="HEADING_CLASSES">{{ REQUEST_ISSUES_HEADING }}</h2>
    <div class="flex flex-col gap-2">
      <button v-for="issue in REQUEST_ISSUES" :key="issue.num" type="button" disabled :class="DISABLED_ITEM_CLASSES">
        <span class="font-mono text-small text-muted">{{ issue.num }}</span> {{ issue.title }}
      </button>
    </div>

    <h2 :class="HEADING_CLASSES">{{ REQUEST_CONTEXT_HEADING }}</h2>
    <div class="flex flex-col gap-2">
      <button v-for="item in REQUEST_CONTEXT" :key="item.label" type="button" disabled :class="DISABLED_ITEM_CLASSES">
        <b class="block text-caption text-ink">{{ item.label }}</b>
        <span class="text-small text-muted">{{ item.hint }}</span>
      </button>
    </div>

    <h2 :class="HEADING_CLASSES">{{ REQUEST_START_HEADING }}</h2>
    <div class="grid grid-cols-2 gap-3">
      <button v-for="option in REQUEST_START_OPTIONS" :key="option.label" type="button" disabled :class="DISABLED_ITEM_CLASSES">
        <b class="block text-caption text-ink">{{ option.label }}</b>
        <span class="text-small text-muted">{{ option.hint }}</span>
      </button>
    </div>

    <h2 :class="HEADING_CLASSES">{{ NEXT_HEADING }}</h2>
    <ol class="flex flex-col gap-1.5 text-caption">
      <li v-for="(item, index) in NEXT_STEPS" :key="item.title">
        <span class="text-muted">{{ index + 1 }}.</span> <b class="text-ink">{{ item.title }}</b>
        <span class="text-muted"> · {{ item.text }}</span>
      </li>
    </ol>

    <p v-if="originKind === 'folder' && linkedRepo" data-testid="linked-repo" class="text-caption text-muted">{{ LINKED_REPO_LABEL }} · {{ linkedRepo }}</p>

    <div v-if="error?.field === 'general'" data-testid="create-error">
      <UiNotice :text="error.message" @close="error = null" />
    </div>

    <div class="flex items-center justify-end gap-2">
      <UiButton variant="quiet" size="lg" data-testid="step-back" @press="wizard.back">{{ BACK_LABEL }}</UiButton>
      <UiButton variant="quiet" size="lg" :disabled="busy" data-testid="step-create" @press="wizard.create">
        {{ CREATE_WITHOUT_REQUEST_LABEL }}
      </UiButton>
      <UiButton variant="primary" size="lg" disabled aria-disabled="true">{{ REQUEST_SEND_LABEL }}</UiButton>
    </div>
  </div>
</template>
