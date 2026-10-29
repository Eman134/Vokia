<script setup lang="ts">
import { computed, ref, type Component } from 'vue'
import { SAMPLE_DATA_LABEL } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import UiButton from '../../ui/UiButton.vue'
import VokiaLogo from '../brand/VokiaLogo.vue'
import BoasVindasStep from './steps/BoasVindasStep.vue'
import ConexoesStep from './steps/ConexoesStep.vue'
import ProjetoStep from './steps/ProjetoStep.vue'
import TimeStep from './steps/TimeStep.vue'
import PedidoStep from './steps/PedidoStep.vue'
import ProntoStep from './steps/ProntoStep.vue'

type StepId = 'boas-vindas' | 'conexoes' | 'projeto' | 'time' | 'pedido' | 'pronto'

interface OnboardingStep {
  id: StepId
  view: Component
  forwardLabel: string
  forwardDisabled?: boolean
  sample: boolean
}

const CONNECT_CLAUDE_ONLY_LABEL = 'Continuar só com Claude'
const STEPS: readonly OnboardingStep[] = [
  { id: 'boas-vindas', view: BoasVindasStep, forwardLabel: 'Começar', sample: false },
  { id: 'conexoes', view: ConexoesStep, forwardLabel: CONNECT_CLAUDE_ONLY_LABEL, sample: true },
  { id: 'projeto', view: ProjetoStep, forwardLabel: 'Continuar', sample: true },
  { id: 'time', view: TimeStep, forwardLabel: 'Continuar', sample: true },
  { id: 'pedido', view: PedidoStep, forwardLabel: 'Enviar para a Iara', forwardDisabled: true, sample: true },
  { id: 'pronto', view: ProntoStep, forwardLabel: 'Abrir o mapa do time', sample: true },
]
const LAST_INDEX = STEPS.length - 1

const ROOT_CLASSES =
  'relative flex min-w-0 flex-1 flex-col items-center overflow-y-auto bg-bg px-6 pt-10 pb-[150px] text-ink'
const MARK_CLASSES = 'grid size-8 place-items-center rounded-control bg-ink'
const WORDMARK_CLASSES = 'text-[18px] font-bold tracking-[-0.02em]'
const SKIP_CLASSES = 'absolute top-[46px] right-10'
const MAIN_CLASSES = 'mt-14 flex w-[min(600px,100%)] flex-col items-center gap-7'
const MARK_LOGO_SIZE = 20
const ARROW_SIZE = 14
const ARROW_PATH = 'M3 8h9M8.5 4.5 12 8l-3.5 3.5'

const navigation = useNavigation()
const index = ref(0)
const current = computed(() => STEPS[index.value])
const isFirst = computed(() => index.value === 0)

function goTo(id: StepId): void {
  index.value = STEPS.findIndex((step) => step.id === id)
}

function back(): void {
  index.value -= 1
}

function forward(): void {
  if (index.value === LAST_INDEX) {
    navigation.goTo('mapa')
    return
  }
  index.value += 1
}
</script>

<template>
  <div :class="ROOT_CLASSES">
    <header class="flex w-full items-center gap-2.5 px-4">
      <span :class="MARK_CLASSES" :style="{ '--logo-cut': 'var(--color-ink)' }">
        <VokiaLogo variant="cheio" :size="MARK_LOGO_SIZE" class="text-bg" />
      </span>
      <b :class="WORDMARK_CLASSES">Vokia</b>
    </header>
    <UiButton
      v-if="current.id !== 'pronto'"
      variant="quiet"
      size="sm"
      :class="SKIP_CLASSES"
      @press="goTo('pronto')"
    >
      Pular e ir para o fim →
    </UiButton>
    <main :class="MAIN_CLASSES">
      <p v-if="current.sample" class="w-full text-caption text-muted">{{ SAMPLE_DATA_LABEL }}</p>
      <component :is="current.view" @skip-team="goTo('pedido')" />
      <footer class="flex w-full items-center gap-2" :class="isFirst ? 'justify-center' : 'justify-between'">
        <UiButton v-if="!isFirst" variant="quiet" size="lg" @press="back">Voltar</UiButton>
        <UiButton variant="primary" size="lg" :disabled="current.forwardDisabled" @press="forward">
          {{ current.forwardLabel }}
          <svg
            :width="ARROW_SIZE"
            :height="ARROW_SIZE"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path :d="ARROW_PATH" />
          </svg>
        </UiButton>
      </footer>
    </main>
  </div>
</template>
