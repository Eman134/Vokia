<script lang="ts">
let mascotInstances = 0
const DEFAULT_SIZE = 64
</script>

<script setup lang="ts">
import { computed } from 'vue'
import { BRAND_GRADIENT } from './brandLogo'
import {
  MASCOT_ART,
  MASCOT_HEAD_ART,
  type MascotHead,
  type MascotLook,
  type MascotPaint,
} from './mascotArt'

const props = withDefaults(defineProps<{ look: MascotLook; size?: number; head?: MascotHead }>(), {
  size: DEFAULT_SIZE,
})

const art = computed(() => (props.head ? MASCOT_HEAD_ART[props.head] : MASCOT_ART[props.look]))

// Id único: dois mascotes na mesma tela não podem compartilhar o gradiente da cabeça.
const gradientId = `vokia-mascot-${mascotInstances++}`

function isToken(paint: MascotPaint | undefined): paint is 'ink' | 'surface' {
  return paint === 'ink' || paint === 'surface'
}

// Atributo de apresentação não aceita var(): tokens do tema entram via style.
function paintAttr(paint: MascotPaint | undefined): string | undefined {
  if (paint === 'brand') return `url(#${gradientId})`
  return isToken(paint) ? undefined : paint
}

function tokenStyle(paint: MascotPaint | undefined): string | undefined {
  return isToken(paint) ? `var(--color-${paint})` : undefined
}
</script>

<template>
  <svg :width="size" :height="size" :viewBox="art.viewBox" aria-hidden="true">
    <defs v-if="head === 'cor'">
      <linearGradient :id="gradientId" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" :stop-color="BRAND_GRADIENT.flame.to" />
        <stop offset=".55" :stop-color="BRAND_GRADIENT.v.from" />
        <stop offset="1" :stop-color="BRAND_GRADIENT.v.to" />
      </linearGradient>
    </defs>
    <component
      :is="shape.tag"
      v-for="(shape, index) in art.parts"
      :key="index"
      v-bind="shape.attrs"
      :fill="paintAttr(shape.fill)"
      :stroke="paintAttr(shape.stroke)"
      :style="{ fill: tokenStyle(shape.fill), stroke: tokenStyle(shape.stroke) }"
      :data-part="shape.part"
    />
  </svg>
</template>
