<script setup lang="ts">
defineProps<{
  columns: readonly { key: string; title: string; align?: 'left' | 'right' }[]
  rows: readonly Record<string, unknown>[]
  minWidth?: number
}>()

const ALIGN_CLASSES: Record<'left' | 'right', string> = { left: 'text-left', right: 'text-right' }
</script>

<template>
  <div class="w-full overflow-x-auto">
    <table
      class="w-full border-collapse text-caption text-ink"
      :style="minWidth ? { minWidth: `${minWidth}px` } : undefined"
    >
      <thead>
        <tr class="border-b border-line">
          <th
            v-for="column in columns"
            :key="column.key"
            scope="col"
            class="px-3 py-2 text-meta font-normal text-muted"
            :class="ALIGN_CLASSES[column.align ?? 'left']"
          >
            {{ column.title }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(row, index) in rows" :key="index" class="border-b border-line">
          <td
            v-for="column in columns"
            :key="column.key"
            class="px-3 py-2 tabular-nums"
            :class="ALIGN_CLASSES[column.align ?? 'left']"
          >
            <slot :name="`cell-${column.key}`" :row="row" :value="row[column.key]">{{ row[column.key] }}</slot>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
