import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { isUserNameValid, useUserProfile } from './useUserProfile'
import type { UserAvatarChoice } from './userIcons'

export const useProfileDraft = defineStore('profileDraft', () => {
  const profile = useUserProfile()
  const name = ref('')
  const avatar = ref<UserAvatarChoice | null>(null)
  const error = ref<string | null>(null)
  const saving = ref(false)
  const valid = computed(() => isUserNameValid(name.value))

  function begin(): void {
    name.value = profile.name ?? ''
    avatar.value = profile.avatar
    error.value = null
  }

  async function submit(): Promise<boolean> {
    if (!valid.value || saving.value) return false
    saving.value = true
    try {
      await profile.save(name.value, avatar.value)
      error.value = null
      return true
    } catch (failure) {
      error.value = failure instanceof Error ? failure.message : String(failure)
      return false
    } finally {
      saving.value = false
    }
  }

  return { name, avatar, error, saving, valid, begin, submit }
})
