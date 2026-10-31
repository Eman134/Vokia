<script setup lang="ts">
import type { SlashCommand } from './slashCommands'

defineProps<{
  id: string
  commands: readonly SlashCommand[]
  activeIndex: number
  left: number
}>()

const emit = defineEmits<{
  choose: [command: SlashCommand]
}>()
</script>

<template>
  <div
    class="absolute bottom-full mb-2 w-75 rounded-xl border border-line bg-surface p-1.25 shadow-float"
    :style="{ left: `${left}px` }"
  >
    <ul v-if="commands.length > 0" :id="id" role="listbox" aria-label="Comandos" class="flex flex-col gap-px">
      <li
        v-for="(command, index) in commands"
        :id="`${id}-${command.id}`"
        :key="command.id"
        role="option"
        :aria-selected="index === activeIndex"
        :aria-disabled="!command.enabled"
        class="flex cursor-pointer items-center gap-2.5 rounded-lg px-2 py-1.5"
        :class="[index === activeIndex ? 'bg-accent-soft' : undefined, command.enabled ? undefined : 'opacity-40']"
        @mousedown.prevent
        @click="emit('choose', command)"
      >
        <span class="w-18.5 font-mono text-label text-ink">/{{ command.id }}</span>
        <span class="text-label text-muted">{{ command.description }}</span>
      </li>
    </ul>
    <p v-else class="px-2 py-1.5 text-caption text-muted">Nenhum comando</p>
  </div>
</template>
