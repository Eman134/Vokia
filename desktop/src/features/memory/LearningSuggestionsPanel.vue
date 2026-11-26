<script setup lang="ts">
import { computed, onMounted, reactive } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { useProjects } from '../projects/useProjects'
import type { LearningSuggestion, SkillScope, SuggestionKind, SuggestionOperation } from './learningApi'
import { useLearningSuggestions } from './useLearningSuggestions'

const props = defineProps<{
  filterKind?: SuggestionKind
  title?: string
}>()

const store = useLearningSuggestions()
const projects = useProjects()
const selectedScopes = reactive<Record<string, SkillScope>>({})

const SCOPE_OPTIONS: readonly { label: string; value: SkillScope }[] = [
  { label: 'Projeto (.vokia/skills)', value: 'project' },
  { label: 'Global (~/.vokia/skills)', value: 'global' },
]

const OPERATION_LABELS: Record<SuggestionOperation, string> = {
  add: 'Adicionar memória',
  update: 'Atualizar',
  forget: 'Remover memória',
  create: 'Criar skill',
}

const KIND_LABELS: Record<SuggestionKind, string> = {
  memory: 'Memória',
  skill: 'SKILL.md',
}

const CARD_CLASSES = 'flex flex-col gap-3 rounded-md border border-line bg-surface p-4'
const HEADER_CLASSES = 'flex flex-wrap items-center justify-between gap-2'
const BADGE_CLASSES =
  'inline-flex items-center rounded-full bg-accent-soft px-2 py-0.5 text-tiny font-semibold text-accent'
const KIND_PILL_CLASSES =
  'inline-flex items-center rounded-sm border border-line bg-surface-2 px-1.5 py-0.5 text-tiny font-medium text-muted'
const DIFF_BEFORE_CLASSES =
  'rounded-sm border border-line bg-surface-2 px-2.5 py-2 font-mono text-small text-muted whitespace-pre-wrap break-words'
const DIFF_AFTER_CLASSES =
  'rounded-sm border border-line bg-ok-soft px-2.5 py-2 font-mono text-small text-ink whitespace-pre-wrap break-words'
const PREVIEW_CLASSES =
  'rounded-sm border border-line bg-surface-2 px-2.5 py-2 font-mono text-small text-ink whitespace-pre-wrap break-words'

const visibleSuggestions = computed<LearningSuggestion[]>(() => {
  const list = store.pendingSuggestions
  if (!props.filterKind) return list
  return list.filter((item) => item.kind === props.filterKind)
})

const currentProjectId = computed(() => projects.selected?.id)

function scopeFor(item: LearningSuggestion): SkillScope {
  return selectedScopes[item.id] ?? item.skillScope ?? 'project'
}

function setScope(id: string, value: string): void {
  selectedScopes[id] = value === 'global' ? 'global' : 'project'
}

function isBusy(id: string): boolean {
  return store.busyIds.has(id)
}

onMounted(() => {
  void store.bindEvents()
  void store.load(currentProjectId.value)
})
</script>

<template>
  <section
    :aria-label="props.title ?? 'Sugestões de aprendizado'"
    class="flex flex-col gap-3 rounded-md border border-line bg-surface p-4"
    data-testid="learning-suggestions-panel"
  >
    <div :class="HEADER_CLASSES">
      <div class="flex items-center gap-2">
        <h2 class="text-lead text-ink">{{ props.title ?? 'Sugestões de aprendizado' }}</h2>
        <span
          v-if="visibleSuggestions.length > 0"
          :class="BADGE_CLASSES"
          data-testid="learning-suggestions-badge"
          :aria-label="`${visibleSuggestions.length} sugestões pendentes`"
        >
          {{ visibleSuggestions.length }}
        </span>
      </div>
      <UiButton
        v-if="currentProjectId !== undefined"
        variant="quiet"
        size="sm"
        :disabled="store.reviewing"
        @press="store.triggerReview(currentProjectId)"
      >
        {{ store.reviewing ? 'Analisando...' : 'Revisar agora' }}
      </UiButton>
    </div>

    <p v-if="store.error" role="alert" class="text-small text-danger">
      {{ store.error }}
    </p>

    <p v-if="store.loading && visibleSuggestions.length === 0" class="text-caption text-muted">
      Carregando sugestões...
    </p>

    <p v-else-if="visibleSuggestions.length === 0" class="text-caption text-muted">
      Nenhuma sugestão pendente no momento.
    </p>

    <ul v-else class="flex flex-col gap-3">
      <li
        v-for="item in visibleSuggestions"
        :key="item.id"
        :class="CARD_CLASSES"
        :data-testid="`learning-suggestion-${item.id}`"
      >
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div class="flex flex-wrap items-center gap-2">
            <span :class="KIND_PILL_CLASSES">{{ KIND_LABELS[item.kind] }}</span>
            <span :class="KIND_PILL_CLASSES">{{ OPERATION_LABELS[item.operation] }}</span>
            <span class="text-control font-semibold text-ink">{{ item.title }}</span>
            <span v-if="item.skillSlug" class="font-mono text-small text-muted">
              {{ item.skillSlug }}
            </span>
          </div>
          <span v-if="item.model" class="font-mono text-tiny text-muted">
            {{ item.providerId }} · {{ item.model }}
          </span>
        </div>

        <p v-if="item.rationale" class="text-caption text-muted">
          {{ item.rationale }}
        </p>

        <div v-if="item.previousContent" class="flex flex-col gap-1.5">
          <span class="text-tiny font-medium text-muted">Antes</span>
          <pre :class="DIFF_BEFORE_CLASSES">{{ item.previousContent }}</pre>
          <span class="text-tiny font-medium text-ok">Depois</span>
          <pre :class="DIFF_AFTER_CLASSES">{{ item.content }}</pre>
        </div>
        <pre v-else :class="PREVIEW_CLASSES">{{ item.content }}</pre>

        <div class="flex flex-wrap items-center justify-between gap-2 pt-1">
          <UiSelect
            v-if="item.kind === 'skill'"
            :model-value="scopeFor(item)"
            :options="SCOPE_OPTIONS"
            label="Escopo da skill"
            @update:model-value="(val) => val && setScope(item.id, val)"
          />
          <span v-else class="text-tiny text-muted">
            {{ item.sourceCardId ? `Card ${item.sourceCardId}` : 'Revisão em background' }}
          </span>

          <div class="ml-auto flex items-center gap-2">
            <UiButton
              variant="quiet"
              size="sm"
              :disabled="isBusy(item.id)"
              @press="store.dismiss(item.id)"
            >
              Descartar
            </UiButton>
            <UiButton
              variant="primary"
              size="sm"
              :disabled="isBusy(item.id)"
              @press="store.apply(item.id, item.kind === 'skill' ? scopeFor(item) : undefined)"
            >
              {{ isBusy(item.id) ? 'Aplicando...' : 'Aplicar' }}
            </UiButton>
          </div>
        </div>
      </li>
    </ul>
  </section>
</template>
