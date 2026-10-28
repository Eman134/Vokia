<script setup lang="ts">
import { computed } from 'vue'
import VokiaLogo from './VokiaLogo.vue'
import { LOGO_VIEWBOX, type LogoSize } from './brandLogo'

const props = defineProps<{ orientation: 'horizontal' | 'vertical'; onDark?: boolean }>()

const LOGO_SIZE_BY_ORIENTATION: Record<'horizontal' | 'vertical', LogoSize> = {
  horizontal: 32,
  vertical: 48,
}
// Largura da língua de fogo no viewBox: área livre mínima ao redor da marca.
const CLEAR_SPACE_UNITS = 4

const logoSize = computed(() => LOGO_SIZE_BY_ORIENTATION[props.orientation])
const logoVariant = computed(() => (props.onDark ? 'cor' : 'cheio'))
const clearSpace = computed(() => `${(logoSize.value * CLEAR_SPACE_UNITS) / LOGO_VIEWBOX}px`)
const wordmarkClass = computed(() => (props.onDark ? 'text-bg' : 'text-ink'))
</script>

<template>
  <div
    :class="['inline-flex items-center gap-2.5', orientation === 'vertical' && 'flex-col']"
    :style="{ padding: clearSpace }"
  >
    <VokiaLogo :variant="logoVariant" :size="logoSize" />
    <span class="text-lead" :class="wordmarkClass">Vokia</span>
  </div>
</template>
