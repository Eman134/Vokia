<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import type { ConsoleEntry } from '../team/events'
import UiButton from '../../ui/UiButton.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { useFollowEnd } from '../../ui/useFollowEnd'
import ConsoleLines from './ConsoleLines.vue'
import { CONSOLE_FILTERS, CONSOLE_FILTER_LABEL, visibleEntries, type ConsoleFilter } from './consoleFilters'

const props = defineProps<{ entries: readonly ConsoleEntry[] }>()
const filter = defineModel<ConsoleFilter>('filter', { required: true })

const JUMP_LABEL = 'Ir para o fim'
const EMPTY_TEXT = 'Nenhuma saída ainda.'

const scroller = ref<HTMLElement | null>(null)
const { followingEnd, onScroll, followEnd, jumpToEnd } = useFollowEnd(scroller)
const visible = computed(() => visibleEntries(props.entries, filter.value))

watch(() => visible.value.length, followEnd)
onMounted(followEnd)

defineExpose({ jumpToEnd })
</script>

<template>
  <div class="px-4 py-2">
    <UiSegmented v-model="filter" :options="CONSOLE_FILTERS" :label="CONSOLE_FILTER_LABEL" />
  </div>
  <slot />
  <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto bg-term-bg px-4 py-3" @scroll="onScroll">
    <ConsoleLines v-if="visible.length > 0" :entries="visible" />
    <p v-else class="font-mono text-small text-term-muted">{{ EMPTY_TEXT }}</p>
  </div>
  <div v-if="!followingEnd" class="px-4 py-2">
    <UiButton variant="primary" size="sm" @press="jumpToEnd">{{ JUMP_LABEL }}</UiButton>
  </div>
</template>
