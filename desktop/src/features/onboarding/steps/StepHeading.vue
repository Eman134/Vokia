<script setup lang="ts">
import { PAGE_TITLE_CLASSES } from '../../../ui/UiPageHeader.vue'
import UiAnnotation from '../../../ui/UiAnnotation.vue'

defineProps<{
  title: string
  text: string
  note?: string
}>()

const TEXT_CLASSES = 'max-w-[46ch] text-lead font-normal leading-normal text-muted'
</script>

<template>
  <div class="flex flex-col items-center gap-3.5 text-center">
    <slot name="mascot" />
    <UiAnnotation v-if="note" :text="note" direction="up" />
    <h1 :class="PAGE_TITLE_CLASSES">{{ title }}</h1>
    <p :class="TEXT_CLASSES">{{ text }}</p>
  </div>
</template>
