<script setup lang="ts">
import { computed } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { ruleLine } from '../orchestration/orchestrationLabels'
import type { Enforcement, OrchLayer, OrchRule } from '../orchestration/orchestrationTypes'
import {
  EDIT_IN_ORCHESTRATION_LABEL,
  FLOW_TITLE,
  NO_RULES_OFF_TEXT,
  NO_RULES_ON_TEXT,
} from './teamMapLayers'

const props = defineProps<{ rules: OrchRule[]; layers: OrchLayer[]; enforcement: Enforcement }>()
const emit = defineEmits<{ edit: [] }>()

const emptyText = computed(() => (props.enforcement === 'on' ? NO_RULES_ON_TEXT : NO_RULES_OFF_TEXT))
</script>

<template>
  <section class="flex flex-col gap-3 rounded-[24px] border border-line bg-surface p-4.5">
    <div class="flex items-center justify-between gap-3">
      <h2 class="text-control font-bold text-ink">{{ FLOW_TITLE }}</h2>
      <UiButton variant="quiet" size="sm" @press="emit('edit')">{{ EDIT_IN_ORCHESTRATION_LABEL }}</UiButton>
    </div>
    <ul v-if="rules.length > 0" class="m-0 flex list-none flex-col gap-1.5 p-0">
      <li v-for="rule in rules" :key="rule.id" class="text-caption text-ink">{{ ruleLine(rule, layers) }}</li>
    </ul>
    <p v-else class="text-body text-muted">{{ emptyText }}</p>
  </section>
</template>
