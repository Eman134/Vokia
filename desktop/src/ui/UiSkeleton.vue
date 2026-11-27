<script setup lang="ts">
import { computed } from 'vue'

export type SkeletonPart =
  | 'heading'
  | 'text'
  | 'subtitle'
  | 'paragraph'
  | 'chip'
  | 'button'
  | 'avatar'
  | 'list-item'
  | 'list-item-two-line'
  | 'list-item-avatar'
  | 'card'
  | 'block'

const props = withDefaults(
  defineProps<{
    type?: string
    count?: number
  }>(),
  {
    type: 'text',
    count: 1,
  },
)

const BONE_CLASSES = 'rounded-sm bg-surface-2 animate-pulse motion-reduce:animate-none'

const parts = computed<SkeletonPart[]>(() =>
  props.type
    .split(',')
    .map((part) => part.trim() as SkeletonPart)
    .filter((part) => part.length > 0),
)
</script>

<template>
  <div
    role="status"
    aria-busy="true"
    aria-label="Carregando…"
    class="flex w-full min-w-0 flex-col gap-2.5"
    data-testid="ui-skeleton"
  >
    <template v-for="iteration in count" :key="iteration">
      <template v-for="(part, partIndex) in parts" :key="`${iteration}-${part}-${partIndex}`">
        <div v-if="part === 'heading'" :class="[BONE_CLASSES, 'h-5 w-44']" />

        <div v-else-if="part === 'text'" :class="[BONE_CLASSES, 'h-3.5 w-full']" />

        <div v-else-if="part === 'subtitle'" :class="[BONE_CLASSES, 'h-3 w-2/3']" />

        <div v-else-if="part === 'paragraph'" class="flex flex-col gap-2">
          <div :class="[BONE_CLASSES, 'h-3.5 w-full']" />
          <div :class="[BONE_CLASSES, 'h-3.5 w-5/6']" />
          <div :class="[BONE_CLASSES, 'h-3.5 w-2/3']" />
        </div>

        <div v-else-if="part === 'chip'" :class="[BONE_CLASSES, 'h-6 w-20 rounded-full']" />

        <div v-else-if="part === 'button'" :class="[BONE_CLASSES, 'h-8 w-24 rounded-md']" />

        <div v-else-if="part === 'avatar'" :class="[BONE_CLASSES, 'size-9 shrink-0 rounded-full']" />

        <div v-else-if="part === 'list-item'" :class="[BONE_CLASSES, 'h-10 w-full rounded-md']" />

        <div
          v-else-if="part === 'list-item-two-line'"
          class="flex flex-col gap-2 rounded-md border border-line bg-surface px-3.5 py-3"
        >
          <div :class="[BONE_CLASSES, 'h-3.5 w-3/4']" />
          <div :class="[BONE_CLASSES, 'h-3 w-1/2']" />
        </div>

        <div
          v-else-if="part === 'list-item-avatar'"
          class="flex items-center gap-3 rounded-md border border-line bg-surface px-3.5 py-3"
        >
          <div :class="[BONE_CLASSES, 'size-8 shrink-0 rounded-full']" />
          <div class="flex min-w-0 flex-1 flex-col gap-1.5">
            <div :class="[BONE_CLASSES, 'h-3.5 w-2/3']" />
            <div :class="[BONE_CLASSES, 'h-3 w-1/2']" />
          </div>
        </div>

        <div
          v-else-if="part === 'card'"
          class="flex flex-col gap-3 rounded-md border border-line bg-surface p-4"
        >
          <div :class="[BONE_CLASSES, 'h-4 w-1/3']" />
          <div class="flex flex-col gap-2">
            <div :class="[BONE_CLASSES, 'h-3.5 w-full']" />
            <div :class="[BONE_CLASSES, 'h-3.5 w-4/5']" />
          </div>
        </div>

        <div v-else :class="[BONE_CLASSES, 'h-32 w-full rounded-md']" />
      </template>
    </template>
  </div>
</template>
