<script setup lang="ts">
import { useConnection } from '../useConnection'
import { BAR_WRAP_CLASSES, PILL_CLASSES } from './barClasses'

const connection = useConnection()
</script>

<template>
  <div role="status" :class="connection.online ? undefined : BAR_WRAP_CLASSES">
    <div v-if="!connection.online" :class="[PILL_CLASSES, 'w-full justify-center']">
      <svg
        class="size-4 shrink-0 text-danger"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M12 19.5h.01" />
        <path d="M8.5 15.5a5 5 0 0 1 7 0" />
        <path d="M5 12a10 10 0 0 1 14 0" />
        <path d="M2 8.8a15 15 0 0 1 20 0" />
        <path d="M3 3l18 18" />
      </svg>
      <span class="text-label font-semibold text-danger">Sem conexão</span>
    </div>
  </div>
</template>
