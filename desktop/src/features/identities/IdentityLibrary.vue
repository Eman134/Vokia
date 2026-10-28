<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import {
  IDENTITY_ESTILOS,
  IDENTITY_FAMILIES,
  IDENTITY_PALETAS,
  IDENTITY_RECURSOS,
  findIdentityConcept,
  type IdentityConcept,
} from './identityFamilies'
import { COMPARE_SIZES, IDENTITY_STATES } from './identityState'
import IdentityGlyph from './IdentityGlyph.vue'

const ALL = 'todos'
const COMPARE_LIMIT = 4
const LIST_GLYPH_SIZE = 32

const FILTERS = [
  { key: 'recurso', label: 'Recurso', options: IDENTITY_RECURSOS },
  { key: 'estilo', label: 'Estilo', options: IDENTITY_ESTILOS },
  { key: 'paleta', label: 'Paleta', options: IDENTITY_PALETAS },
] as const

const COMPARE_BACKGROUNDS = [
  { id: 'claro', surface: 'bg-surface', text: 'text-ink' },
  { id: 'escuro', surface: 'bg-ink', text: 'text-bg' },
] as const

interface CompareRef {
  family: string
  concept: string
}

const filters = reactive({ recurso: ALL, estilo: ALL, paleta: ALL })
const selected = ref<CompareRef[]>([])

const matchesFilters = (concept: IdentityConcept) =>
  FILTERS.every(({ key }) => filters[key] === ALL || concept[key] === filters[key])

const visibleFamilies = computed(() =>
  IDENTITY_FAMILIES.map((family) => ({ ...family, concepts: family.concepts.filter(matchesFilters) })).filter(
    (family) => family.concepts.length > 0,
  ),
)

const compareItems = computed(() =>
  selected.value.map((ref) => ({ ...ref, label: findIdentityConcept(ref.family, ref.concept).label })),
)

const isSame = (a: CompareRef, familyId: string, conceptId: string) =>
  a.family === familyId && a.concept === conceptId

const isSelected = (familyId: string, conceptId: string) =>
  selected.value.some((ref) => isSame(ref, familyId, conceptId))

function toggleCompare(familyId: string, conceptId: string) {
  if (isSelected(familyId, conceptId)) {
    selected.value = selected.value.filter((ref) => !isSame(ref, familyId, conceptId))
    return
  }
  selected.value = [...selected.value.slice(-(COMPARE_LIMIT - 1)), { family: familyId, concept: conceptId }]
}

function clearAll() {
  for (const { key } of FILTERS) filters[key] = ALL
  selected.value = []
}
</script>

<template>
  <section class="mx-auto flex max-w-[1500px] flex-col gap-9 p-6">
    <header class="flex flex-col gap-2">
      <h1 class="text-title font-semibold text-ink">Identidades visuais</h1>
      <p class="max-w-[72ch] text-lead text-muted">
        Uma biblioteca de avatares, emblemas e símbolos para agentes, projetos, documentos, equipes, fluxos e
        recursos. Cada conceito tem uma família, variações e marcas de estado que não dependem só de cor.
      </p>
    </header>

    <div class="flex flex-wrap items-center gap-3">
      <label v-for="filter in FILTERS" :key="filter.key" class="flex items-center gap-2 text-caption text-muted">
        {{ filter.label }}
        <select
          v-model="filters[filter.key]"
          class="h-8 rounded-control border border-line bg-surface px-2 text-control text-ink"
        >
          <option :value="ALL">Todos</option>
          <option v-for="(text, value) in filter.options" :key="value" :value="value">{{ text }}</option>
        </select>
      </label>
      <button
        type="button"
        class="h-8 rounded-control border border-line bg-surface px-3 text-control text-ink"
        @click="clearAll"
      >
        Limpar
      </button>
    </div>

    <section v-if="compareItems.length" class="flex flex-col gap-4">
      <div class="flex flex-wrap items-baseline gap-3">
        <h2 class="text-section text-ink">Comparar</h2>
        <span class="text-caption text-muted">
          {{ compareItems.length }} de {{ COMPARE_LIMIT }} escolhidos · tamanhos {{ COMPARE_SIZES.join(', ') }} em
          fundo claro e escuro, estados e variações
        </span>
      </div>
      <div class="grid grid-cols-2 gap-3 max-md:grid-cols-1">
        <div
          v-for="background in COMPARE_BACKGROUNDS"
          :key="background.id"
          class="flex flex-col gap-5 rounded-card border border-line p-4"
          :class="background.surface"
        >
          <div v-for="item in compareItems" :key="`${item.family}/${item.concept}`" class="flex flex-col gap-2">
            <span class="text-control font-semibold" :class="background.text">{{ item.label }}</span>
            <div v-for="size in COMPARE_SIZES" :key="size" class="flex flex-wrap items-center gap-3">
              <IdentityGlyph
                v-for="state in IDENTITY_STATES"
                :key="state"
                :family="item.family"
                :concept="item.concept"
                :state="state"
                :size="size"
              />
            </div>
          </div>
        </div>
      </div>
    </section>

    <section v-for="family in visibleFamilies" :key="family.id" class="flex flex-col gap-3">
      <h2 class="text-section text-ink">{{ family.label }}</h2>
      <ul class="flex flex-col gap-2">
        <li v-for="concept in family.concepts" :key="concept.id">
          <button
            type="button"
            class="flex w-full flex-wrap items-center gap-4 rounded-card border p-3 text-left"
            :class="isSelected(family.id, concept.id) ? 'border-accent bg-accent-soft' : 'border-line bg-surface'"
            :aria-pressed="isSelected(family.id, concept.id)"
            @click="toggleCompare(family.id, concept.id)"
          >
            <span class="w-40 text-control font-semibold text-ink">{{ concept.label }}</span>
            <span class="flex flex-wrap items-start gap-4">
              <span v-for="state in IDENTITY_STATES" :key="state" class="flex flex-col items-center gap-1">
                <IdentityGlyph
                  :family="family.id"
                  :concept="concept.id"
                  :state="state"
                  :size="LIST_GLYPH_SIZE"
                />
                <span class="text-caption text-muted">{{ state }}</span>
              </span>
            </span>
          </button>
        </li>
      </ul>
    </section>
  </section>
</template>
