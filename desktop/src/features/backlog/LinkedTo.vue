<script setup lang="ts">
import { computed } from 'vue'
import type { BacklogLink } from './backlogTypes'
import { linkKind } from './planBreakdown'

const props = defineProps<{
  itemId: string
  hasPlan: boolean
  links?: readonly BacklogLink[]
}>()

const emit = defineEmits<{
  'open-plan': []
  'open-link': [link: BacklogLink]
}>()

const CHIP_CLASSES = 'shrink-0 rounded-chip bg-surface-2 px-1.5 py-0.5 font-mono text-tiny text-muted'
const LINK_BUTTON_CLASSES =
  'flex w-full cursor-pointer items-center gap-2 rounded-control px-1.5 py-1 text-left text-caption text-ink transition-colors hover:bg-hover focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-accent'

const visible = computed(() => props.hasPlan || (props.links?.length ?? 0) > 0)
</script>

<template>
  <section v-if="visible" data-testid="linked-to">
    <h3 class="mb-1.5 text-label font-semibold text-ink">Ligado a</h3>
    <ul class="flex flex-col gap-1">
      <li v-if="hasPlan">
        <button type="button" :class="LINK_BUTTON_CLASSES" @click="emit('open-plan')">
          <span :class="CHIP_CLASSES">plano</span>
          <span class="min-w-0 flex-1 truncate">plano-{{ itemId }}</span>
        </button>
      </li>
      <li v-for="link in links" :key="link.id">
        <button type="button" :class="LINK_BUTTON_CLASSES" @click="emit('open-link', link)">
          <span :class="CHIP_CLASSES">{{ linkKind(link.id) }}</span>
          <span class="min-w-0 flex-1 truncate">{{ link.label }}</span>
        </button>
      </li>
    </ul>
  </section>
</template>
