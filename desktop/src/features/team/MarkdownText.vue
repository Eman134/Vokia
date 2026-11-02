<script setup lang="ts">
import { computed } from 'vue'
import { renderMarkdown } from './markdown'

const props = withDefaults(defineProps<{ text: string; mentions?: readonly string[] }>(), {
  mentions: () => [],
})

// renderMarkdown escapa todo o texto antes de montar o HTML, então v-html recebe só marcação nossa.
const html = computed(() => renderMarkdown(props.text, props.mentions))
</script>

<template>
  <div
    class="wrap-break-word flex flex-col gap-2 text-control text-ink [&_a]:text-accent [&_a]:underline [&_blockquote]:border-l-2 [&_blockquote]:border-line [&_blockquote]:pl-3 [&_blockquote]:text-muted [&_code]:rounded-sm [&_code]:bg-surface-2 [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-small [&_del]:text-muted [&_em]:italic [&_h3]:font-semibold [&_h4]:font-semibold [&_h5]:font-semibold [&_hr]:border-line [&_ol]:list-decimal [&_ol]:pl-5 [&_pre]:overflow-x-auto [&_pre]:rounded-card [&_pre]:border [&_pre]:border-line [&_pre]:bg-surface [&_pre]:p-3 [&_pre_code]:bg-transparent [&_pre_code]:p-0 [&_strong]:font-semibold [&_ul]:list-disc [&_ul]:pl-5"
    v-html="html"
  />
</template>
