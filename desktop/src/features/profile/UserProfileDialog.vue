<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { trapFocus } from '../backlog/dialogFocus'
import { useNavigation } from '../../shell/useNavigation'
import UserProfileForm from './UserProfileForm.vue'
import { useProfileDraft } from './useProfileDraft'
import { useUserProfile } from './useUserProfile'

const NAME_FIELD_SELECTOR = 'input'

const profile = useUserProfile()
const draft = useProfileDraft()
const navigation = useNavigation()
const titleId = useId()
const dialog = ref<HTMLElement>()
const postponed = ref(false)
const isOpen = computed(() => profile.needsProfile && !postponed.value && navigation.page !== 'onboarding')

watch(
  isOpen,
  async (open) => {
    if (!open) return
    draft.begin()
    await nextTick()
    dialog.value?.querySelector<HTMLElement>(NAME_FIELD_SELECTOR)?.focus()
  },
  { immediate: true },
)

async function save(): Promise<void> {
  await draft.submit()
}
</script>

<template>
  <div v-if="isOpen" class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4">
    <section
      ref="dialog"
      data-testid="profile-dialog"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      tabindex="-1"
      class="flex max-h-full w-120 max-w-full flex-col items-center gap-8 overflow-y-auto rounded-dialog bg-surface p-8 shadow-float"
      @keydown.esc.stop="postponed = true"
      @keydown.tab="trapFocus($event, dialog)"
    >
      <b :id="titleId" class="text-center text-lead text-ink">Como devemos te chamar?</b>
      <UserProfileForm />
      <div class="flex justify-center gap-3">
        <UiButton variant="quiet" size="lg" @press="postponed = true">Agora não</UiButton>
        <UiButton variant="primary" size="lg" :disabled="!draft.valid || draft.saving" @press="save">Salvar</UiButton>
      </div>
    </section>
  </div>
</template>
