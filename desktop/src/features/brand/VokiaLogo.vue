<script lang="ts">
let logoInstances = 0
</script>

<script setup lang="ts">
import {
  BRAND_GRADIENT,
  LOGO_DROP_CUT_PATH,
  LOGO_DROP_PATH,
  LOGO_FLAME_CORE_PATH,
  LOGO_FLAME_CUT_PATH,
  LOGO_FLAME_PATH,
  LOGO_V_PATH,
  LOGO_VIEWBOX,
  type LogoSize,
  type LogoVariant,
} from './brandLogo'

defineProps<{ variant: LogoVariant; size: LogoSize }>()

const OUTLINE_ATTRS = {
  fill: 'none',
  stroke: 'currentColor',
  'stroke-width': 3,
  'stroke-linejoin': 'round',
} as const
// Recorte usa a cor de fundo do pai: sem ele o miolo some ou fica escuro demais.
const CUT_STYLE = { fill: 'var(--logo-cut, var(--color-bg))' }

const GRADIENT_ID_PREFIX = 'vokia-logo'

const gradientId = `${GRADIENT_ID_PREFIX}-${logoInstances++}`
const vGradientId = `${gradientId}-v`
const flameGradientId = `${gradientId}-flame`
</script>

<template>
  <svg
    :width="size"
    :height="size"
    :viewBox="`0 0 ${LOGO_VIEWBOX} ${LOGO_VIEWBOX}`"
    aria-hidden="true"
    :data-variant="variant"
  >
    <template v-if="variant === 'cheio'">
      <path :d="LOGO_V_PATH" fill="currentColor" />
      <path :d="LOGO_FLAME_PATH" fill="currentColor" />
      <path :d="LOGO_FLAME_CUT_PATH" :style="CUT_STYLE" />
    </template>
    <template v-else-if="variant === 'contorno'">
      <path :d="LOGO_V_PATH" v-bind="OUTLINE_ATTRS" />
      <path :d="LOGO_FLAME_PATH" v-bind="OUTLINE_ATTRS" />
    </template>
    <template v-else-if="variant === 'gota'">
      <path :d="LOGO_DROP_PATH" fill="currentColor" />
      <path :d="LOGO_DROP_CUT_PATH" :style="CUT_STYLE" />
    </template>
    <template v-else>
      <defs>
        <linearGradient :id="vGradientId" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" :stop-color="BRAND_GRADIENT.v.from" />
          <stop offset="1" :stop-color="BRAND_GRADIENT.v.to" />
        </linearGradient>
        <linearGradient :id="flameGradientId" x1="0" y1="1" x2="0" y2="0">
          <stop offset="0" :stop-color="BRAND_GRADIENT.flame.from" />
          <stop offset="1" :stop-color="BRAND_GRADIENT.flame.to" />
        </linearGradient>
      </defs>
      <path :d="LOGO_V_PATH" :fill="`url(#${vGradientId})`" />
      <path :d="LOGO_FLAME_PATH" :fill="`url(#${flameGradientId})`" />
      <path :d="LOGO_FLAME_CORE_PATH" :fill="BRAND_GRADIENT.core" opacity=".9" />
    </template>
  </svg>
</template>
