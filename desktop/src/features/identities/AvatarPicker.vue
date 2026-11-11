<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import {
  CANCEL_PROFILE_LABEL,
  EXPRESSION_LABELS,
  NEW_PROFILE_LABEL,
  PROFILE_COLOR_LABEL,
  PROFILE_EXPRESSION_LABEL,
  PROFILE_NAME_LABEL,
  PROFILE_PREVIEW_LABEL,
  PROFILE_SHAPE_LABEL,
  SAVE_PROFILE_LABEL,
} from './avatarPickerCopy'
import { AVATAR_COLORS, AVATAR_SHAPES, renderAvatarArt, type AvatarExpression, type AvatarShapeId } from './avatarProfiles'
import { AvatarProfileError, useAvatarProfiles } from './useAvatarProfiles'

const AVATAR_SIZE = 40
const PREVIEW_SIZE = 56
const PREVIEW_PROFILE_ID = 'previa'
const DEFAULT_SHAPE: AvatarShapeId = 'gota'
const DEFAULT_EXPRESSION: AvatarExpression = 'feliz'

defineProps<{ labelId: string }>()
const selected = defineModel<string>({ required: true })

const store = useAvatarProfiles()
const group = ref<HTMLElement>()
const creator = ref<HTMLElement>()
const addButton = ref<HTMLElement>()
const creating = ref(false)
const draftName = ref('')
const draftShape = ref<AvatarShapeId>(DEFAULT_SHAPE)
const draftColor = ref(AVATAR_COLORS[0].id)
const draftExpression = ref<AvatarExpression>(DEFAULT_EXPRESSION)
const creatorError = ref<string | null>(null)
const saving = ref(false)

const shapeOptions = AVATAR_SHAPES.map((shape) => ({ label: shape.label, value: shape.id }))
const shapeExpressions = computed(
  () => Object.keys(AVATAR_SHAPES.find((shape) => shape.id === draftShape.value)?.faces ?? {}) as AvatarExpression[],
)
const expressionOptions = computed(() =>
  shapeExpressions.value.map((expression) => ({ label: EXPRESSION_LABELS[expression], value: expression })),
)
const previewArt = computed(() =>
  renderAvatarArt({
    id: PREVIEW_PROFILE_ID,
    nome: draftName.value,
    forma: draftShape.value,
    cor: draftColor.value,
    expressao: draftExpression.value,
  }),
)

watch(draftShape, () => {
  if (!shapeExpressions.value.includes(draftExpression.value)) draftExpression.value = DEFAULT_EXPRESSION
})

function moveAvatar(event: KeyboardEvent, step: number): void {
  event.preventDefault()
  const profiles = store.all
  const index = profiles.findIndex((profile) => profile.id === selected.value)
  const next = profiles[(index + step + profiles.length) % profiles.length]
  selected.value = next.id
  group.value?.querySelector<HTMLElement>(`[data-avatar="${next.id}"]`)?.focus()
}

async function openCreator(): Promise<void> {
  creating.value = true
  await nextTick()
  creator.value?.querySelector('input')?.focus()
}

function closeCreator(): void {
  creating.value = false
  draftName.value = ''
  draftShape.value = DEFAULT_SHAPE
  draftColor.value = AVATAR_COLORS[0].id
  draftExpression.value = DEFAULT_EXPRESSION
  creatorError.value = null
}

async function cancelCreator(): Promise<void> {
  closeCreator()
  await nextTick()
  addButton.value?.focus()
}

async function dismissLoadNotice(): Promise<void> {
  store.dismissLoadNotice()
  await nextTick()
  const active = group.value?.querySelector<HTMLElement>(`[data-avatar="${selected.value}"]`)
  ;(active ?? group.value?.querySelector<HTMLElement>('[data-avatar]'))?.focus()
}

async function dismissCreatorError(): Promise<void> {
  creatorError.value = null
  await nextTick()
  creator.value?.querySelector('input')?.focus()
}

async function saveProfile(): Promise<void> {
  if (saving.value) return
  saving.value = true
  creatorError.value = null
  try {
    const profile = await store.create({
      nome: draftName.value,
      forma: draftShape.value,
      cor: draftColor.value,
      expressao: draftExpression.value,
    })
    selected.value = profile.id
    closeCreator()
    await nextTick()
    group.value?.querySelector<HTMLElement>(`[data-avatar="${profile.id}"]`)?.focus()
  } catch (error) {
    creatorError.value = error instanceof AvatarProfileError ? error.message : String(error)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="flex flex-col gap-2">
    <UiNotice v-if="store.loadNotice !== null" :text="store.loadNotice" @close="dismissLoadNotice()" />
    <div class="flex flex-wrap gap-1.5">
    <div ref="group" role="radiogroup" :aria-labelledby="labelId" class="contents">
      <button
        v-for="profile in store.all"
        :key="profile.id"
        type="button"
        role="radio"
        :data-avatar="profile.id"
        :aria-checked="selected === profile.id"
        :aria-label="profile.nome"
        :tabindex="selected === profile.id ? 0 : -1"
        class="rounded-control p-1 focus:outline-2 focus:outline-solid focus:outline-accent"
        :class="selected === profile.id ? 'bg-accent-soft ring-2 ring-accent' : 'border border-line'"
        @click="selected = profile.id"
        @keydown.right="moveAvatar($event, 1)"
        @keydown.down="moveAvatar($event, 1)"
        @keydown.left="moveAvatar($event, -1)"
        @keydown.up="moveAvatar($event, -1)"
      >
        <svg :width="AVATAR_SIZE" :height="AVATAR_SIZE" viewBox="0 0 64 64" aria-hidden="true">
          <g v-html="renderAvatarArt(profile)" />
        </svg>
      </button>
    </div>
    <button
      ref="addButton"
      type="button"
      :aria-label="NEW_PROFILE_LABEL"
      :disabled="creating"
      class="grid size-12.5 place-items-center rounded-control border border-dashed border-line text-control text-muted focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40"
      @click="openCreator()"
    >
      +
    </button>
    </div>
    <div
      v-if="creating"
      ref="creator"
      class="flex flex-col gap-3 rounded-control border border-line p-3"
      @keydown.esc.stop="cancelCreator()"
    >
      <div @keydown.enter.prevent="saveProfile()">
        <UiField v-model="draftName" :label="PROFILE_NAME_LABEL" />
      </div>
      <UiSegmented v-model="draftShape" :options="shapeOptions" :label="PROFILE_SHAPE_LABEL" wrap />
      <div role="group" :aria-label="PROFILE_COLOR_LABEL" class="flex flex-wrap gap-1.5">
        <button
          v-for="color in AVATAR_COLORS"
          :key="color.id"
          type="button"
          :aria-pressed="draftColor === color.id"
          :aria-label="color.label"
          :style="{ backgroundColor: color.base }"
          class="size-6 rounded-full focus:outline-2 focus:outline-solid focus:outline-accent"
          :class="draftColor === color.id ? 'ring-2 ring-accent ring-offset-2' : 'border border-line'"
          @click="draftColor = color.id"
        />
      </div>
      <UiSegmented v-model="draftExpression" :options="expressionOptions" :label="PROFILE_EXPRESSION_LABEL" wrap />
      <svg :width="PREVIEW_SIZE" :height="PREVIEW_SIZE" viewBox="0 0 64 64" role="img" :aria-label="PROFILE_PREVIEW_LABEL">
        <g v-html="previewArt" />
      </svg>
      <UiNotice v-if="creatorError !== null" :text="creatorError" @close="dismissCreatorError()" />
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="cancelCreator()">{{ CANCEL_PROFILE_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" :disabled="draftName.trim() === '' || saving" @press="saveProfile()">
          {{ SAVE_PROFILE_LABEL }}
        </UiButton>
      </div>
    </div>
  </div>
</template>
