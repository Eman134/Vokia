<script lang="ts">
let portraitInstances = 0
</script>

<script setup lang="ts">
import { computed } from 'vue'
import {
  DEFAULT_PORTRAIT,
  DEFAULT_PORTRAIT_SIZE,
  PORTRAIT_IDS,
  PORTRAIT_META,
  PORTRAIT_TRAITS,
  type HairStyle,
} from './portraitArt'

const props = withDefaults(defineProps<{ who: string; size?: number }>(), {
  size: DEFAULT_PORTRAIT_SIZE,
})

const INK = '#18181b'

// Id único: dois retratos do mesmo agente na tela não podem disputar o mesmo recorte circular.
const clipId = `vokia-portrait-${portraitInstances++}`

const id = computed(() => PORTRAIT_IDS.find((portrait) => portrait === props.who.toLowerCase()) ?? DEFAULT_PORTRAIT)
const meta = computed(() => PORTRAIT_META[id.value])
const traits = computed(() => PORTRAIT_TRAITS[id.value])
const label = computed(() => `Retrato de ${meta.value.name}, ${meta.value.role}`)

function hasHair(style: HairStyle): boolean {
  return traits.value.hairStyles.includes(style)
}
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 64 64" role="img" :aria-label="label" class="rounded-control">
    <defs>
      <clipPath :id="clipId">
        <circle cx="32" cy="32" r="32" />
      </clipPath>
    </defs>
    <g :clip-path="`url(#${clipId})`">
      <rect width="64" height="64" :fill="traits.bg" />
      <path d="M8 64 C10 49 21 44 32 44 C43 44 54 49 56 64 Z" :fill="traits.shirt" />
      <rect x="28" y="36" width="8" height="10" rx="2" :fill="traits.skin" />
      <path
        v-if="hasHair('long')"
        d="M20 30 C18 14 26 11 32 11 C40 11 46 15 44 30 L47 46 L40 42 L41 26 C38 20 26 20 23 26 L24 42 L17 46 Z"
        :fill="traits.hair"
      />
      <path
        v-if="hasHair('bob')"
        d="M19 37 C17 15 26 11 32 11 C39 11 47 15 45 37 L40 37 L41 24 C36 19 28 19 23 24 L24 37 Z"
        :fill="traits.hair"
      />
      <ellipse cx="32" cy="28" rx="11" ry="12.5" :fill="traits.skin" />
      <path
        v-if="hasHair('short')"
        d="M20.5 27 C19.5 14 27 11.5 32 11.5 C39 11.5 45 14.5 43.5 27 C41 19.5 25 19 20.5 27 Z"
        :fill="traits.hair"
      />
      <g v-if="hasHair('curly')" :fill="traits.hair">
        <circle cx="23" cy="20" r="5" />
        <circle cx="28" cy="15.5" r="5.5" />
        <circle cx="34.5" cy="14.5" r="5.5" />
        <circle cx="40.5" cy="18.5" r="5" />
        <circle cx="43" cy="24" r="3.5" />
        <circle cx="21" cy="25" r="3.5" />
      </g>
      <circle v-if="hasHair('bun')" cx="32" cy="9.5" r="5.5" :fill="traits.hair" />
      <path
        v-if="hasHair('buzz')"
        d="M21.5 25 C22 16 27 14.5 32 14.5 C37 14.5 42 16 42.5 25 C38 21 26 21 21.5 25 Z"
        :fill="traits.hair"
      />
      <path
        v-if="traits.beard"
        d="M22 29 C23 40 28 42.5 32 42.5 C36 42.5 41 40 42 29 C38 35.5 26 35.5 22 29 Z"
        :fill="traits.hair"
      />
      <circle cx="28" cy="29" r="1.3" :fill="INK" />
      <circle cx="36" cy="29" r="1.3" :fill="INK" />
      <path d="M29 34.2 Q32 36.6 35 34.2" fill="none" :stroke="INK" stroke-width="1.3" stroke-linecap="round" />
      <g v-if="traits.glasses" fill="none" :stroke="INK" stroke-width="1.3">
        <circle cx="28" cy="29" r="3.8" />
        <circle cx="36" cy="29" r="3.8" />
        <path d="M31.8 29 L32.2 29" />
      </g>
      <template v-if="traits.cap">
        <path d="M20 23 C21 13 43 13 44 23 Z" :fill="traits.cap" />
        <rect x="30" y="21.5" width="20" height="3" rx="1.5" :fill="traits.cap" />
      </template>
      <g v-if="traits.phones">
        <path d="M18.5 29 C18.5 10 45.5 10 45.5 29" fill="none" :stroke="INK" stroke-width="2.6" />
        <rect x="16" y="25" width="5.5" height="10" rx="2.5" :fill="INK" />
        <rect x="42.5" y="25" width="5.5" height="10" rx="2.5" :fill="INK" />
      </g>
    </g>
  </svg>
</template>
