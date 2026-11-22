<script setup lang="ts">
import { computed, ref } from 'vue'
import { useChannels } from './useChannels'
import type { Channel } from './channelsApi'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import AgentPortrait from '../team/AgentPortrait.vue'
import { PORTRAIT_SIZES } from '../team/portraitArt'
import UiNotice from '../../ui/UiNotice.vue'

import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
const MIN_MEMBERS = 2
const NAME_MIN_CHARS = 2
const NAME_MAX_CHARS = 32
const INVALID_NAME_CHARS = /[^a-z0-9-]/g
const WHITESPACE = /\s+/g
const CREATE_LABEL = 'Criar canal'
const SAVE_LABEL = 'Salvar alterações'
const CANCEL_LABEL = 'Cancelar'
const SELECT_ALL_LABEL = 'Selecionar todos'
const CLEAR_LABEL = 'Limpar seleção'
const NAME_RULES = 'Letras minúsculas, números e hífens, sem espaços.'
const SECTION_LABEL_CLASSES = 'text-caption font-semibold text-ink'
const NAME_FIELD_CLASSES =
  'flex h-10 items-center gap-1 rounded-md border border-line bg-surface-2 px-3 transition-[border-color,box-shadow] duration-150 focus-within:border-accent focus-within:ring-2 focus-within:ring-accent/30 motion-reduce:transition-none'
const MEMBER_CLASSES =
  'group flex cursor-pointer items-center gap-3 rounded-md border border-line bg-surface p-2 transition-colors duration-150 hover:bg-hover has-[:checked]:border-accent has-[:checked]:bg-accent-soft has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-solid has-[:focus-visible]:outline-accent motion-reduce:transition-none'
const CHECK_CLASSES =
  'grid size-5 shrink-0 place-items-center rounded-full border border-line text-transparent transition-colors duration-150 group-has-[:checked]:border-accent group-has-[:checked]:bg-accent group-has-[:checked]:text-accent-ink motion-reduce:transition-none'
const ACTION_BASE_CLASSES =
  `inline-flex h-8 cursor-pointer items-center gap-1.5 rounded-md px-2.5 text-caption font-medium transition-colors ${INK_FOCUS_CLASSES} disabled:cursor-not-allowed disabled:opacity-40`
const CANCEL_CLASSES = `${ACTION_BASE_CLASSES} text-muted hover:bg-hover hover:text-ink`
const SUBMIT_CLASSES = `${ACTION_BASE_CLASSES} border border-line bg-surface text-ink enabled:hover:bg-hover`
const LINK_CLASSES =
  `cursor-pointer rounded-sm px-1.5 py-0.5 text-caption font-medium text-accent hover:bg-accent-soft ${INK_FOCUS_CLASSES}`

const props = defineProps<{ channel?: Channel }>()
const emit = defineEmits<{ done: []; cancel: [] }>()

const channels = useChannels()
const roster = useRoster()
const name = ref(props.channel?.name ?? '')
const members = ref<Agent[]>([...(props.channel?.members ?? [])])
const busy = ref(false)
const failure = ref<string | null>(null)

const allSelected = computed(() => roster.agents.length > 0 && members.value.length === roster.agents.length)
const membersEnough = computed(() => members.value.length >= MIN_MEMBERS)
const canSubmit = computed(() => name.value.length >= NAME_MIN_CHARS && membersEnough.value && !busy.value)
const membersSummary = computed(() => {
  const count = members.value.length
  return `${count} ${count === 1 ? 'agente selecionado' : 'agentes selecionados'} · mínimo ${MIN_MEMBERS}`
})

function normalizeName(raw: string): string {
  return raw.toLowerCase().replace(WHITESPACE, '-').replace(INVALID_NAME_CHARS, '')
}

function onNameInput(event: Event): void {
  const input = event.target as HTMLInputElement
  name.value = normalizeName(input.value)
  input.value = name.value
}

function toggleAll(): void {
  members.value = allSelected.value ? [] : roster.agents.map((agent) => agent.id)
}

async function submit(): Promise<void> {
  if (!canSubmit.value) return
  failure.value = null
  busy.value = true
  try {
    if (props.channel) {
      await channels.update(props.channel.id, name.value, members.value)
    } else {
      await channels.create(name.value, members.value)
    }
    emit('done')
  } catch (error) {
    failure.value = String(error)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <form class="flex min-h-0 flex-col gap-5" @submit.prevent="submit">
    <div class="flex flex-col gap-1.5">
      <label for="channel-name" :class="SECTION_LABEL_CLASSES">Nome do canal</label>
      <div :class="NAME_FIELD_CLASSES">
        <span class="text-control font-semibold text-muted" aria-hidden="true">#</span>
        <input
          id="channel-name"
          type="text"
          :value="name"
          :maxlength="NAME_MAX_CHARS"
          autocomplete="off"
          spellcheck="false"
          placeholder="ex: squad-frontend"
          class="min-w-0 flex-1 bg-transparent text-control text-ink outline-none placeholder:text-faint"
          @input="onNameInput"
        />
        <span class="text-tiny tabular-nums text-faint">{{ name.length }}/{{ NAME_MAX_CHARS }}</span>
      </div>
      <p class="text-caption text-muted">{{ NAME_RULES }}</p>
    </div>
    <fieldset class="flex min-h-0 flex-col gap-2">
      <div class="flex items-center justify-between gap-2">
        <legend :class="SECTION_LABEL_CLASSES">Agentes participantes</legend>
        <button type="button" :class="LINK_CLASSES" @click="toggleAll">
          {{ allSelected ? CLEAR_LABEL : SELECT_ALL_LABEL }}
        </button>
      </div>
      <ul class="flex max-h-60 min-h-0 flex-col gap-1.5 overflow-y-auto pr-0.5">
        <li v-for="agent in roster.agents" :key="agent.id">
          <label :class="MEMBER_CLASSES">
            <input v-model="members" type="checkbox" class="sr-only" :value="agent.id" />
            <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZES.stack" />
            <span class="flex min-w-0 flex-1 flex-col">
              <span class="truncate text-label font-semibold text-ink">{{ agent.name }}</span>
              <span class="truncate text-tiny text-muted">{{ agent.role }}</span>
            </span>
            <span :class="CHECK_CLASSES" aria-hidden="true">
              <svg class="size-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                <path d="M5 12l5 5L20 7" />
              </svg>
            </span>
          </label>
        </li>
      </ul>
      <p :class="['text-caption', membersEnough ? 'text-ok' : 'text-muted']" aria-live="polite">{{ membersSummary }}</p>
    </fieldset>
    <UiNotice v-if="failure" :text="failure" @close="failure = null" />
    <div class="flex items-center justify-end gap-2 border-t border-line pt-4">
      <button type="button" :class="CANCEL_CLASSES" @click="emit('cancel')">{{ CANCEL_LABEL }}</button>
      <button type="submit" :class="SUBMIT_CLASSES" :disabled="!canSubmit">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path v-if="channel" d="M5 12l5 5L20 7" />
          <template v-else>
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </template>
        </svg>
        <span>{{ channel ? SAVE_LABEL : CREATE_LABEL }}</span>
      </button>
    </div>
  </form>
</template>
