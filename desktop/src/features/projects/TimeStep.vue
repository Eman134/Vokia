<script setup lang="ts">
import AgentPortrait from '../team/AgentPortrait.vue'
import UiButton from '../../ui/UiButton.vue'
import {
  BACK_LABEL,
  CONTINUE_TO_REQUEST_LABEL,
  SAMPLE_AGENTS,
  SAMPLE_AGENTS_COUNT,
  SAMPLE_AGENTS_HEADING,
  SAMPLE_LAYERS,
  SAMPLE_LAYERS_HEADING,
  SAMPLE_LAYERS_HINT,
  SAMPLE_ORCH_HEADING,
  SAMPLE_ORCH_OPTIONS,
  SAMPLE_REUSE_MODES,
  SAMPLE_SHARE_HEADING,
  SAMPLE_SHARE_OPTIONS,
  SAMPLE_TEAMS,
  SAMPLE_TEAMS_HEADING,
  TEAM_FOOTER_HINT,
} from './projetosSample'
import { useNovoProjeto } from './useNovoProjeto'

const PORTRAIT_SIZE = 32
const HEADING_CLASSES = 'text-label font-medium text-muted'
const TAB_GROUP_CLASSES = 'flex w-fit gap-0.5 rounded-control bg-surface-2 p-0.75 opacity-40'
const TAB_CLASSES = 'whitespace-nowrap rounded-control-sm px-3 py-1.25 text-caption'
const CARD_CLASSES = 'flex flex-col gap-1 rounded-[18px] border border-line bg-surface p-3.5 text-left opacity-60'
const OPTION_CLASSES = 'flex flex-col gap-0.5 rounded-control border border-line bg-surface px-3 py-2 text-left opacity-60'
const ROW_CLASSES = 'flex items-center gap-3 rounded-control border border-line px-3 py-2 opacity-60'

const wizard = useNovoProjeto()
</script>

<template>
  <div class="flex flex-col gap-5.5" aria-disabled="true">
    <div role="group" aria-label="Modo do time" aria-disabled="true" :class="TAB_GROUP_CLASSES">
      <button
        v-for="(label, index) in SAMPLE_REUSE_MODES"
        :key="label"
        type="button"
        disabled
        :class="[TAB_CLASSES, index === 0 ? 'bg-surface font-medium text-ink' : 'text-muted']"
      >
        {{ label }}
      </button>
    </div>

    <h2 :class="HEADING_CLASSES">{{ SAMPLE_TEAMS_HEADING }}</h2>
    <div class="grid grid-cols-3 gap-3">
      <button v-for="team in SAMPLE_TEAMS" :key="team.name" type="button" disabled :class="CARD_CLASSES">
        <b class="text-ink">{{ team.name }}</b>
        <span class="text-small text-muted">{{ team.from }}</span>
        <span class="text-small text-muted">{{ team.desc }}</span>
      </button>
    </div>

    <h2 :class="HEADING_CLASSES">{{ SAMPLE_SHARE_HEADING }}</h2>
    <div class="grid grid-cols-2 gap-3">
      <button v-for="option in SAMPLE_SHARE_OPTIONS" :key="option.label" type="button" disabled :class="OPTION_CLASSES">
        <b class="text-caption text-ink">{{ option.label }}</b>
        <span class="text-small text-muted">{{ option.hint }}</span>
      </button>
    </div>

    <h2 :class="HEADING_CLASSES">{{ SAMPLE_AGENTS_HEADING }} · {{ SAMPLE_AGENTS_COUNT }}</h2>
    <div class="flex flex-col gap-2">
      <div v-for="agent in SAMPLE_AGENTS" :key="agent.id" :class="ROW_CLASSES">
        <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZE" />
        <div class="min-w-0 flex-1">
          <b class="block text-ink">{{ agent.name }}</b>
          <span class="text-small text-muted">{{ agent.role }}</span>
        </div>
        <span class="text-small text-muted">{{ agent.cli }} · {{ agent.layer }}</span>
        <span class="text-small text-muted">{{ agent.load }}</span>
      </div>
    </div>

    <h2 :class="HEADING_CLASSES">{{ SAMPLE_ORCH_HEADING }}</h2>
    <div class="grid grid-cols-3 gap-3">
      <button v-for="option in SAMPLE_ORCH_OPTIONS" :key="option.label" type="button" disabled :class="OPTION_CLASSES">
        <b class="text-caption text-ink">{{ option.label }}</b>
        <span class="text-small text-muted">{{ option.hint }}</span>
      </button>
    </div>

    <h2 :class="HEADING_CLASSES">{{ SAMPLE_LAYERS_HEADING }}</h2>
    <p class="text-caption text-ink">{{ SAMPLE_LAYERS.join(' · ') }}</p>
    <p class="text-small text-muted">{{ SAMPLE_LAYERS_HINT }}</p>

    <div class="flex items-center justify-end gap-3">
      <UiButton variant="quiet" size="lg" data-testid="step-back" @press="wizard.back">{{ BACK_LABEL }}</UiButton>
      <span class="text-small text-muted">{{ TEAM_FOOTER_HINT }}</span>
      <UiButton variant="primary" size="lg" data-testid="step-continue" @press="wizard.next">
        {{ CONTINUE_TO_REQUEST_LABEL }}
      </UiButton>
    </div>
  </div>
</template>
