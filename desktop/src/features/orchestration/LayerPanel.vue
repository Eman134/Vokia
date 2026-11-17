<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiField from '../../ui/UiField.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { NAME_LABEL } from '../agents/agentFormCopy'
import { DESCRIPTION_LABEL, TYPE_LABEL } from '../teammap/teamMapLayers'
import { LAYER_KIND_LABELS, VIEW_COPY } from './orchestrationLabels'
import { LAYER_KIND_VALUES, type LayerKind, type OrchLayer } from './orchestrationTypes'
import { useOrchestration } from './useOrchestration'

const INSTRUCTIONS_MAX_CHARS = 4000
const TEXTAREA_CLASSES =
  'w-full min-w-0 rounded-control border border-line bg-surface px-3 py-2 text-control text-ink placeholder:text-faint focus:outline-2 focus:outline-solid focus:outline-accent'
const SIDE_MOVE = { previous: -1, next: 1 } as const

const props = defineProps<{ layer: OrchLayer; orderedIds: string[] }>()
const emit = defineEmits<{ close: [] }>()

const orchestration = useOrchestration()
const kindOptions = LAYER_KIND_VALUES.map((value) => ({ label: LAYER_KIND_LABELS[value], value }))

const name = ref(props.layer.name)
const description = ref(props.layer.description)
const kind = ref<string>(props.layer.kind)
const instructions = ref(props.layer.instructions)

const canSave = computed(() => name.value.trim() !== '' && kind.value !== '')
const instructionsCount = computed(() => `${instructions.value.length} / ${INSTRUCTIONS_MAX_CHARS}`)

const position = computed(() => props.orderedIds.indexOf(props.layer.id))
const canMovePrevious = computed(() => position.value > 0)
const canMoveNext = computed(() => position.value >= 0 && position.value < props.orderedIds.length - 1)

function save(): void {
  if (!canSave.value) return
  void orchestration.saveLayer({
    id: props.layer.id,
    name: name.value.trim(),
    description: description.value.trim(),
    kind: kind.value as LayerKind,
    instructions: instructions.value,
  })
}

function move(direction: number): void {
  const ids = [...props.orderedIds]
  const target = position.value + direction
  const [current] = ids.splice(position.value, 1)
  ids.splice(target, 0, current)
  void orchestration.reorderLayers(ids)
}

function remove(): void {
  void orchestration.deleteLayer(props.layer.id)
}
</script>

<template>
  <section class="flex min-w-0 flex-col gap-4 rounded-[24px] border border-line bg-surface p-4.5">
    <header class="flex items-center justify-between gap-3">
      <h3 class="text-control font-bold text-ink">{{ VIEW_COPY.layerTitle }}</h3>
      <UiButton variant="quiet" size="sm" @press="emit('close')">{{ VIEW_COPY.close }}</UiButton>
    </header>

    <UiField v-model="name" :label="NAME_LABEL" />
    <UiField v-model="description" :label="DESCRIPTION_LABEL" />
    <label class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ TYPE_LABEL }}</span>
      <UiSelect v-model="kind" :options="kindOptions" :label="TYPE_LABEL" />
    </label>
    <label class="flex flex-col gap-1.5">
      <span class="flex items-baseline justify-between gap-3 text-label font-medium text-muted">
        {{ VIEW_COPY.instructions }}
        <span class="font-mono text-tiny font-normal text-faint">{{ instructionsCount }}</span>
      </span>
      <textarea v-model="instructions" rows="5" :class="TEXTAREA_CLASSES" />
    </label>

    <div class="flex flex-wrap items-center gap-2">
      <UiButton variant="primary" size="lg" :disabled="!canSave" @press="save()">{{ VIEW_COPY.saveLayer }}</UiButton>
      <UiButton variant="quiet" size="lg" :disabled="!canMovePrevious" @press="move(SIDE_MOVE.previous)">
        {{ VIEW_COPY.moveLeft }}
      </UiButton>
      <UiButton variant="quiet" size="lg" :disabled="!canMoveNext" @press="move(SIDE_MOVE.next)">
        {{ VIEW_COPY.moveRight }}
      </UiButton>
      <UiConfirm
        class="ml-auto"
        :label="VIEW_COPY.deleteLayer"
        :message="VIEW_COPY.deleteLayerConfirm"
        :confirm-label="VIEW_COPY.deleteLayer"
        @confirm="remove()"
      />
    </div>
  </section>
</template>
