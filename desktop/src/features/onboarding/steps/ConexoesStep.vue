<script setup lang="ts">
import UiButton from '../../../ui/UiButton.vue'
import { CLI_SAMPLES, type CliStatus } from '../onboardingSample'
import StepHeading from './StepHeading.vue'

const CARD_CLASSES = 'flex items-center gap-3.5 rounded-card border border-line bg-surface px-4 py-3.5'
const ICON_CLASSES =
  'grid size-10 flex-none place-items-center rounded-control-lg bg-surface-2 font-mono text-[13px] font-medium'
const STATUS_TEXT_CLASSES: Record<CliStatus, string> = { connected: 'text-ok', pending: 'text-warn-text' }
const STATUS_DOT_CLASSES: Record<CliStatus, string> = { connected: 'bg-ok', pending: 'bg-warn' }
</script>

<template>
  <div class="flex w-full flex-col gap-7">
    <StepHeading
      title="Conecte seus agentes"
      text="A Vokia usa os CLIs que já estão no seu computador. O login é sempre o deles."
    />
    <div class="flex flex-col gap-2.5">
      <div v-for="cli in CLI_SAMPLES" :key="cli.abbr" :class="CARD_CLASSES">
        <span :class="ICON_CLASSES">{{ cli.abbr }}</span>
        <div class="flex min-w-0 flex-1 flex-col gap-0.5">
          <b class="text-[14.5px] font-semibold">{{ cli.name }}</b>
          <span class="flex items-center gap-1.5 text-caption">
            <span :class="['size-[7px] rounded-full', STATUS_DOT_CLASSES[cli.status]]" />
            <span class="font-medium" :class="STATUS_TEXT_CLASSES[cli.status]">{{ cli.statusLabel }}</span>
          </span>
        </div>
        <UiButton v-if="cli.status === 'pending'" variant="primary" size="lg" disabled>Entrar</UiButton>
      </div>
      <span class="text-center text-caption text-muted">Git e GitHub CLI também estão prontos.</span>
    </div>
  </div>
</template>
