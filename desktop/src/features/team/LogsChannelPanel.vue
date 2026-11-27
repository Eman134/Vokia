<script setup lang="ts">
import { computed, ref } from 'vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiButton from '../../ui/UiButton.vue'
import UiIcon from '../../ui/UiIcon.vue'
import { KIND_LABELS as DECISION_KIND_LABELS } from '../decisions/decisionCopy'
import { agentName } from './agents'
import UsageLimitFailedNotice from './UsageLimitFailedNotice.vue'
import { useLogs, type LogEntry, type LogKind } from './useLogs'

type LogFilter = 'all' | 'unread' | LogKind

const FILTERS: ReadonlyArray<{ id: LogFilter; label: string }> = [
  { id: 'all', label: 'Todos' },
  { id: 'unread', label: 'Não lidos' },
  { id: 'error', label: 'Erros' },
  { id: 'decision', label: 'Decisões' },
  { id: 'popup', label: 'Popups' },
]

const KIND_LABELS: Record<LogKind, string> = {
  error: 'Erro',
  decision: 'Decisão',
  popup: 'Popup',
}

const KIND_BADGE_CLASSES: Record<LogKind, string> = {
  error: 'bg-danger/15 text-danger',
  decision: 'bg-accent/15 text-accent',
  popup: 'bg-surface-2 text-muted',
}

const MARK_ALL_READ_LABEL = 'Marcar todos como lidos'
const MARK_READ_LABEL = 'Marcar como lido'
const MARK_UNREAD_LABEL = 'Marcar como não lido'
const READ_LABEL = 'Lido'
const EMPTY_FILTER_TEXT = 'Nenhum log nesta visualização.'

const ERROR_ICON_PATH = 'M8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2z M8 5v3.5M8 10.8v.4'
const CHECK_ICON_PATH = 'M3.5 8.5 6.5 11.5 12.5 4.5'
const TIME_FORMAT = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })

const logs = useLogs()
const filter = ref<LogFilter>('all')

const filteredItems = computed<LogEntry[]>(() => {
  if (filter.value === 'all') return logs.items
  if (filter.value === 'unread') return logs.items.filter((item) => !logs.isRead(item.id))
  return logs.items.filter((item) => item.kind === filter.value)
})

function formatTime(ms: number): string {
  return TIME_FORMAT.format(ms)
}
</script>

<template>
  <div v-if="logs.items.length > 0" class="flex w-full min-w-0 flex-col gap-3" data-testid="logs-panel">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="flex flex-wrap items-center gap-1" role="group" aria-label="Filtrar logs">
        <button
          v-for="option in FILTERS"
          :key="option.id"
          type="button"
          class="cursor-pointer rounded-full border px-2.5 py-0.5 text-tiny font-medium transition-colors"
          :class="[
            INK_FOCUS_CLASSES,
            filter === option.id
              ? 'border-ink bg-ink text-bg'
              : 'border-line bg-surface text-muted hover:text-ink',
          ]"
          :aria-pressed="filter === option.id"
          :data-testid="`logs-filter-${option.id}`"
          @click="filter = option.id"
        >
          {{ option.label }}
        </button>
      </div>
      <UiButton
        v-if="logs.unreadCount > 0"
        variant="quiet"
        size="sm"
        data-testid="logs-mark-all-read"
        @press="logs.markAllRead()"
      >
        {{ MARK_ALL_READ_LABEL }}
      </UiButton>
    </div>

    <p v-if="filteredItems.length === 0" class="py-4 text-center text-caption text-muted">
      {{ EMPTY_FILTER_TEXT }}
    </p>

    <ol v-else class="flex w-full min-w-0 flex-col gap-2">
      <li
        v-for="item in filteredItems"
        :key="item.id"
        class="flex flex-col gap-1.5 rounded-md border px-3 py-2 text-caption transition-opacity"
        :class="[
          item.kind === 'error' && !logs.isRead(item.id)
            ? 'border-danger/35 bg-danger/5'
            : 'border-line bg-surface',
          logs.isRead(item.id) ? 'opacity-65' : '',
        ]"
        :data-testid="`log-item-${item.kind}`"
        :data-log-read="logs.isRead(item.id) ? 'true' : 'false'"
      >
        <div class="flex items-center justify-between gap-2">
          <div class="flex min-w-0 items-center gap-1.5">
            <span
              v-if="!logs.isRead(item.id)"
              class="size-1.5 shrink-0 rounded-full"
              :class="item.kind === 'error' ? 'bg-danger' : 'bg-accent'"
              aria-hidden="true"
            />
            <span
              class="rounded-full px-1.5 py-0.5 text-tiny font-semibold"
              :class="KIND_BADGE_CLASSES[item.kind]"
            >
              {{ KIND_LABELS[item.kind] }}
            </span>
            <span v-if="item.kind === 'decision'" class="truncate text-tiny text-muted">
              {{ agentName(item.agent) }} · {{ DECISION_KIND_LABELS[item.decisionKind] }}
            </span>
            <span v-else-if="item.kind === 'popup'" class="truncate text-tiny text-muted">
              {{ item.source }}
            </span>
            <span class="text-tiny text-muted tabular-nums">{{ formatTime(item.time) }}</span>
          </div>

          <button
            type="button"
            class="inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-sm px-1.5 py-0.5 text-tiny font-medium text-muted hover:bg-hover hover:text-ink"
            :class="INK_FOCUS_CLASSES"
            :aria-label="logs.isRead(item.id) ? MARK_UNREAD_LABEL : MARK_READ_LABEL"
            :title="logs.isRead(item.id) ? MARK_UNREAD_LABEL : MARK_READ_LABEL"
            data-testid="log-toggle-read"
            @click="logs.toggleRead(item.id)"
          >
            <UiIcon :path="CHECK_ICON_PATH" :size="12" />
            <span>{{ logs.isRead(item.id) ? READ_LABEL : MARK_READ_LABEL }}</span>
          </button>
        </div>

        <template v-if="item.kind === 'error'">
          <UsageLimitFailedNotice
            v-if="item.entry.type === 'usage-limit-failed'"
            :entry="item.entry"
          />
          <div v-else class="flex items-start gap-2 text-danger" data-testid="log-error-entry">
            <UiIcon class="mt-0.5 shrink-0" :path="ERROR_ICON_PATH" :size="16" />
            <span class="min-w-0 flex-1 wrap-break-word">{{ item.entry.message }}</span>
          </div>
        </template>

        <div v-else-if="item.kind === 'decision'" class="flex flex-col gap-0.5 text-ink">
          <p class="font-medium wrap-break-word">{{ item.title }}</p>
          <p v-if="item.detail" class="text-muted wrap-break-word">{{ item.detail }}</p>
          <p v-if="item.answer" class="text-tiny text-ok">Respondida: {{ item.answer }}</p>
          <p v-else-if="item.undone" class="text-tiny text-muted">Resposta desfeita (pendente)</p>
        </div>

        <div v-else class="flex flex-col gap-0.5 text-ink">
          <p class="font-medium wrap-break-word">{{ item.title }}</p>
          <p v-if="item.text" class="text-muted wrap-break-word">{{ item.text }}</p>
        </div>
      </li>
    </ol>
  </div>
</template>
