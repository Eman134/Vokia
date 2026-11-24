<script setup lang="ts">
import { computed } from 'vue'
import type { UnreadState } from './unread'

const UNREAD_LABEL = 'Mensagens não lidas'
const DECISION_LABEL = 'Decisão pendente'
const DOT_CLASSES = 'size-1.5 shrink-0 rounded-full'

const props = defineProps<{ state: UnreadState }>()

const label = computed(() => (props.state === 'decision' ? DECISION_LABEL : UNREAD_LABEL))
</script>

<template>
  <span
    v-if="state !== 'read'"
    role="img"
    :class="[DOT_CLASSES, state === 'decision' ? 'bg-danger' : 'bg-accent']"
    :aria-label="label"
    :title="label"
  />
</template>
