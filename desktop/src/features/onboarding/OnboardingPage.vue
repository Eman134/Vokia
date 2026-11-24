<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch, type Component } from 'vue'
import { useNavigation } from '../../shell/useNavigation'
import UiButton from '../../ui/UiButton.vue'
import UiSketchBox from '../../ui/UiSketchBox.vue'
import UiStepDots from '../../ui/UiStepDots.vue'
import VokiaLogo from '../brand/VokiaLogo.vue'
import { useFlow } from '../team/useFlow'
import { useProfileDraft } from '../profile/useProfileDraft'
import { useUserProfile } from '../profile/useUserProfile'
import BoasVindasStep from './steps/BoasVindasStep.vue'
import NomeStep from './steps/NomeStep.vue'
import FotoStep from './steps/FotoStep.vue'
import ProvedoresStep from './steps/ProvedoresStep.vue'
import ProntoStep from './steps/ProntoStep.vue'
import { useOnboardingLlm } from './useOnboardingLlm'

type StepId = 'boas-vindas' | 'nome' | 'foto' | 'provedores' | 'pronto'

interface OnboardingStep {
  id: StepId
  view: Component
  forwardLabel: string
}

const STEPS: readonly OnboardingStep[] = [
  { id: 'boas-vindas', view: BoasVindasStep, forwardLabel: 'Começar' },
  { id: 'nome', view: NomeStep, forwardLabel: 'Continuar' },
  { id: 'foto', view: FotoStep, forwardLabel: 'Continuar' },
  { id: 'provedores', view: ProvedoresStep, forwardLabel: 'Continuar' },
  { id: 'pronto', view: ProntoStep, forwardLabel: 'Abrir projetos' },
]
// Último passo de perfil: é onde nome e foto são salvos juntos.
const SAVE_STEP_ID: StepId = 'foto'
const LAST_INDEX = STEPS.length - 1

const ROOT_CLASSES =
  'relative flex min-w-0 flex-1 flex-col items-center overflow-y-auto bg-bg px-6 pt-10 pb-bar-clearance text-ink'
const MARK_CLASSES = 'grid size-8 place-items-center text-ink'
const WORDMARK_CLASSES = 'text-card-title font-bold'
const MAIN_CLASSES = 'mt-14 flex w-[min(600px,100%)] flex-col items-center gap-7'
const MARK_LOGO_SIZE = 20
const ARROW_SIZE = 14
const ARROW_PATH = 'M3 8h9M8.5 4.5 12 8l-3.5 3.5'

const navigation = useNavigation()
const profile = useUserProfile()
const profileDraft = useProfileDraft()
const onboardingLlm = useOnboardingLlm()
profileDraft.begin()
const index = ref(firstStepIndex())
const flow = useFlow()
flow.start('onboarding', index.value + 1)
const current = computed(() => STEPS[index.value])
const isFirst = computed(() => index.value === 0)
const forwardDisabled = computed(() => {
  if (current.value.id === 'nome') return !profileDraft.nameValid
  return current.value.id === SAVE_STEP_ID && !profileDraft.valid
})

watch(index, (value) => flow.goTo(value + 1))
watch(
  () => flow.step,
  (value) => {
    index.value = value - 1
  },
)
onBeforeUnmount(() => flow.finish())

function firstStepIndex(): number {
  const pending = profile.pendingStep
  return pending === null ? 0 : STEPS.findIndex((step) => step.id === pending)
}

function back(): void {
  index.value -= 1
}

async function forward(): Promise<void> {
  if (current.value.id === SAVE_STEP_ID && !(await profileDraft.submit())) return
  if (current.value.id === 'provedores') {
    try {
      await onboardingLlm.persist()
    } catch {
      // Não bloqueia conclusão do onboarding se settings ainda não persistir
    }
  }
  if (index.value === LAST_INDEX) {
    navigation.goTo('projetos')
    return
  }
  index.value += 1
}
</script>

<template>
  <div :class="ROOT_CLASSES">
    <header class="flex w-full items-center gap-2.5 px-4">
      <UiSketchBox bold>
        <span :class="MARK_CLASSES" :style="{ '--logo-cut': 'var(--color-paper)' }">
          <VokiaLogo variant="cheio" :size="MARK_LOGO_SIZE" />
        </span>
      </UiSketchBox>
      <b :class="WORDMARK_CLASSES">Vokia</b>
    </header>
    <main :class="MAIN_CLASSES">
      <UiStepDots :total="STEPS.length" :current="index" />
      <component :is="current.view" />
      <footer class="flex w-full items-center gap-2" :class="isFirst ? 'justify-center' : 'justify-between'">
        <UiButton v-if="!isFirst" variant="quiet" size="lg" @press="back">Voltar</UiButton>
        <UiButton variant="primary" size="lg" :disabled="forwardDisabled" @press="forward">
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
