<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiChip from '../../ui/UiChip.vue'
import { colorVar, STATE_BADGE, TAG_COLOR, TAG_LABEL, TAG_TEXT_COLOR } from './backlogLabels'
import {
  AUTO_COLUMNS_HINT,
  DEFAULT_COLUMN,
  MAIN_ACTION,
  MODAL_STATE_CHIP,
  MOVABLE_COLUMNS,
  OTHER_OPTION,
  PLAN_HINT,
  type BacklogColumn,
  type BacklogTag,
} from './backlogTypes'
import MicroTaskScript, { isScriptValid } from './MicroTaskScript.vue'
import PlanPanel from './PlanPanel.vue'
import { useBacklog } from './useBacklog'
import { useItemPlan } from './useItemPlan'

const TAG_TINT_PERCENT = 13
const TRIAGE_PLAN_TEXT = 'Plano sai da triagem'
const FOCUSABLE_SELECTOR = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'

const emit = defineEmits<{ replay: []; 'main-action': [] }>()

const backlog = useBacklog()
const itemPlan = useItemPlan()
const titleId = useId()
const dialog = ref<HTMLElement>()
const addingTag = ref(false)
const expandedMicro = ref(new Set<string>())
let previousFocus: HTMLElement | null = null

const item = computed(() => backlog.openItem)
const column = computed(() => item.value?.column ?? DEFAULT_COLUMN)
const moveTargets = computed(() => MOVABLE_COLUMNS.filter((target) => target !== column.value))
const showAutoHint = computed(() => column.value === 'In progress' || column.value === 'In review')
const unusedTags = computed(() =>
  (Object.keys(TAG_LABEL) as BacklogTag[]).filter((tag) => !item.value?.tags.includes(tag)),
)
const visibleMicro = computed(() => item.value?.micro?.filter((task) => isScriptValid(task.script)) ?? [])
const planState = computed(() => (item.value ? (itemPlan.states[item.value.id]?.state ?? item.value.state) : 'vazio'))
const planText = computed(() => (planState.value === 'triagem' ? TRIAGE_PLAN_TEXT : item.value?.plan))
const details = computed(() => {
  const current = item.value
  if (!current) return []
  const rows: [string, string | undefined][] = [
    ['Tipo', current.type === 'feature' ? 'Feature' : 'Tarefa'],
    ['Aberto por', current.by],
    ['Feature', current.feature],
    ['Prioridade', current.priority],
    ['Estimativa', current.estimate],
  ]
  return rows.filter((row): row is [string, string] => Boolean(row[1]))
})

function tagPillStyle(tag: BacklogTag) {
  const token = colorVar(TAG_COLOR[tag])
  return {
    color: colorVar(TAG_TEXT_COLOR[tag] ?? TAG_COLOR[tag]),
    backgroundColor: `color-mix(in srgb, ${token} ${TAG_TINT_PERCENT}%, transparent)`,
  }
}

// O botão acionado sai do DOM com a ação; sem isso o foco cai no body e Esc/Tab deixam de valer.
function changeTag(tag: BacklogTag): void {
  dialog.value?.focus()
  backlog.toggleTag(item.value!.id, tag)
}

function moveTo(target: BacklogColumn): void {
  dialog.value?.focus()
  backlog.moveTo(item.value!.id, target)
}

function toggleMicro(id: string): void {
  const next = new Set(expandedMicro.value)
  if (!next.delete(id)) next.add(id)
  expandedMicro.value = next
}

function trapFocus(event: KeyboardEvent): void {
  const focusable = [...(dialog.value?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR) ?? [])]
  const first = focusable[0]
  const last = focusable[focusable.length - 1]
  if (!first || !last) return
  if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog.value)) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

watch(
  () => backlog.openId,
  async (id) => {
    addingTag.value = false
    if (id === null) {
      previousFocus?.focus()
      previousFocus = null
      return
    }
    const firstMicro = visibleMicro.value[0]
    expandedMicro.value = new Set(firstMicro ? [firstMicro.id] : [])
    previousFocus ??= document.activeElement instanceof HTMLElement ? document.activeElement : null
    await nextTick()
    dialog.value?.focus()
  },
  { immediate: true },
)
</script>

<template>
  <div
    v-if="item"
    class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4"
    @click.self="backlog.close()"
  >
    <section
      ref="dialog"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      class="flex max-h-full w-180 max-w-full flex-col gap-5 overflow-y-auto rounded-dialog border border-line bg-surface p-6 shadow-float"
      @keydown.esc.stop="backlog.close()"
      @keydown.tab="trapFocus"
    >
      <header class="flex flex-col gap-2">
        <div class="flex items-center gap-2 text-caption text-muted">
          <span>{{ item.id }}</span>
          <UiBadge :status="STATE_BADGE[planState]" :label="MODAL_STATE_CHIP[planState]" />
          <UiButton class="ml-auto" variant="quiet" size="sm" @press="backlog.close()">Fechar</UiButton>
        </div>
        <h2 :id="titleId" class="wrap-break-word text-section text-ink">{{ item.title }}</h2>
      </header>

      <section class="flex flex-wrap items-center gap-1.5" aria-label="Etiquetas">
        <span
          v-for="tag in item.tags"
          :key="tag"
          class="inline-flex h-5.5 items-center gap-1 rounded-chip px-2 text-meta font-medium whitespace-nowrap"
          :style="tagPillStyle(tag)"
        >
          {{ TAG_LABEL[tag] }}
          <button type="button" :aria-label="`Remover ${TAG_LABEL[tag]}`" @click="changeTag(tag)">×</button>
        </span>
        <UiChip kind="add" label="+ etiqueta" @press="addingTag = !addingTag" />
      </section>
      <div v-if="addingTag" class="flex flex-wrap items-center gap-1.5">
        <UiChip
          v-for="tag in unusedTags"
          :key="tag"
          kind="add"
          :label="`+ ${TAG_LABEL[tag]}`"
          @press="changeTag(tag)"
        />
        <span class="text-caption text-muted">ou digite uma nova</span>
      </div>

      <section v-if="item.description">
        <h3 class="mb-1 text-label font-semibold text-ink">Descrição</h3>
        <p class="whitespace-pre-wrap text-control text-ink">{{ item.description }}</p>
      </section>

      <section v-if="item.questions?.length" class="flex flex-col gap-3">
        <h3 class="text-label font-semibold text-ink">Perguntas da triagem</h3>
        <div v-for="question in item.questions" :key="question.id" class="flex flex-col gap-1.5">
          <p class="text-control text-ink">{{ question.text }}</p>
          <div class="flex flex-wrap gap-1.5">
            <UiChip
              v-for="option in [...question.options, OTHER_OPTION]"
              :key="option"
              :label="option"
              :selected="question.selected === option"
              @press="backlog.answer(item.id, question.id, option)"
            />
          </div>
        </div>
      </section>

      <section v-if="visibleMicro.length" class="flex flex-col gap-2">
        <h3 class="text-label font-semibold text-ink">Desdobramento</h3>
        <div v-for="task in visibleMicro" :key="task.id" class="flex flex-col gap-2 rounded-control-lg border border-line p-3">
          <button
            type="button"
            class="flex items-center gap-2 text-left text-control text-ink"
            :aria-expanded="expandedMicro.has(task.id)"
            @click="toggleMicro(task.id)"
          >
            <span aria-hidden="true">{{ expandedMicro.has(task.id) ? '▾' : '▸' }}</span>
            <span class="font-medium">{{ task.title }}</span>
            <span class="ml-auto text-caption text-muted">{{ task.who }} · {{ task.model }} · {{ task.status }}</span>
          </button>
          <MicroTaskScript v-if="expandedMicro.has(task.id)" :script="task.script" />
        </div>
      </section>

      <section v-if="item.criteria?.length">
        <h3 class="mb-1 text-label font-semibold text-ink">Critérios de aceite</h3>
        <ul class="flex flex-col gap-1">
          <li v-for="criterion in item.criteria" :key="criterion.id">
            <label class="flex items-center gap-2 text-control text-ink">
              <input type="checkbox" :checked="criterion.done" @change="backlog.toggleCriterion(item.id, criterion.id)" />
              <span :class="criterion.done && 'text-muted line-through'">{{ criterion.text }}</span>
            </label>
          </li>
        </ul>
      </section>

      <section v-if="item.activity?.length">
        <h3 class="mb-1 text-label font-semibold text-ink">Atividade</h3>
        <ul class="flex flex-col gap-1 text-caption text-muted">
          <li v-for="entry in item.activity" :key="entry.id">{{ entry.at }} · {{ entry.text }}</li>
        </ul>
      </section>

      <section>
        <h3 class="mb-1 text-label font-semibold text-ink">Detalhes</h3>
        <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-caption">
          <template v-for="[label, value] in details" :key="label">
            <dt class="text-muted">{{ label }}</dt>
            <dd class="text-ink">{{ value }}</dd>
          </template>
        </dl>
      </section>

      <section class="flex flex-col gap-1">
        <h3 class="text-label font-semibold text-ink">Plano</h3>
        <p v-if="planText" class="whitespace-pre-wrap text-control text-ink">{{ planText }}</p>
        <p class="text-caption text-muted">{{ PLAN_HINT[planState] }}</p>
        <PlanPanel :item="item" />
      </section>

      <section v-if="item.links?.length">
        <h3 class="mb-1 text-label font-semibold text-ink">Vínculos</h3>
        <ul class="flex flex-col gap-1 text-caption text-muted">
          <li v-for="link in item.links" :key="link.id">{{ link.label }}</li>
        </ul>
      </section>

      <section>
        <h3 class="mb-1 text-label font-semibold text-ink">Mover para</h3>
        <div class="flex flex-wrap items-center gap-1.5">
          <UiChip
            v-for="target in moveTargets"
            :key="target"
            kind="add"
            :label="target"
            @press="moveTo(target)"
          />
          <span v-if="showAutoHint" class="text-caption text-muted">{{ AUTO_COLUMNS_HINT }}</span>
        </div>
      </section>

      <footer class="flex items-center justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="emit('replay')">Ver replay</UiButton>
        <UiButton variant="primary" size="lg" @press="emit('main-action')">{{ MAIN_ACTION[planState] }}</UiButton>
      </footer>
    </section>
  </div>
</template>
