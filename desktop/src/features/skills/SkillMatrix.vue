<script setup lang="ts">
import AgentPortrait from '../team/AgentPortrait.vue'
import {
  SKILL_MATRIX_OCCUPATIONS,
  SKILL_MATRIX_ROWS,
  SKILL_MATRIX_SKILL_HEADER,
  SKILL_MATRIX_TITLE,
} from './skillsSample'

const PORTRAIT_SIZE = 22
const PANEL_CLASSES = 'overflow-hidden rounded-dialog border border-line bg-surface'
const TITLE_CLASSES = 'px-5 pt-4.5 pb-1.5 text-lead font-semibold text-ink'
const TABLE_CLASSES = 'w-full min-w-[820px] border-collapse text-label text-ink'
const HEAD_CELL_CLASSES = 'px-2 py-2.5 text-left font-medium text-muted first:pl-5 last:pr-5'
const HEAD_CONTENT_CLASSES = 'inline-flex items-center gap-1.5'
const ROW_CLASSES = 'border-t border-line'
const SKILL_CELL_CLASSES = 'py-2.75 pl-5 pr-2 font-mono text-caption'
const CELL_CLASSES = 'px-2 py-2.75'
const MARKER_CLASSES = 'inline-block size-3.5 rounded-full'
const MARKER_ON_CLASSES = 'bg-accent'
const MARKER_OFF_CLASSES = 'border-[1.5px] border-line'
const MARKER_LABELS = { on: 'Ligada', off: 'Desligada' } as const
</script>

<template>
  <section :class="PANEL_CLASSES" :aria-label="SKILL_MATRIX_TITLE">
    <h2 :class="TITLE_CLASSES">{{ SKILL_MATRIX_TITLE }}</h2>
    <div class="overflow-x-auto">
      <table :class="TABLE_CLASSES">
        <thead>
          <tr class="text-left">
            <th scope="col" :class="HEAD_CELL_CLASSES">{{ SKILL_MATRIX_SKILL_HEADER }}</th>
            <th v-for="occupation in SKILL_MATRIX_OCCUPATIONS" :key="occupation.who" scope="col" :class="HEAD_CELL_CLASSES">
              <span :class="HEAD_CONTENT_CLASSES">
                <AgentPortrait :who="occupation.who" :size="PORTRAIT_SIZE" />
                {{ occupation.title }}
              </span>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in SKILL_MATRIX_ROWS" :key="row.name" :class="ROW_CLASSES">
            <td :class="SKILL_CELL_CLASSES">{{ row.name }}</td>
            <td v-for="(used, index) in row.uses" :key="index" :class="CELL_CLASSES">
              <span
                role="img"
                aria-disabled="true"
                :aria-label="used ? MARKER_LABELS.on : MARKER_LABELS.off"
                :class="[MARKER_CLASSES, used ? MARKER_ON_CLASSES : MARKER_OFF_CLASSES]"
              />
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
