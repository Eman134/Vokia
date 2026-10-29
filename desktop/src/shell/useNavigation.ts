import { ref } from 'vue'
import { defineStore } from 'pinia'
import { DEFAULT_PAGE, type AppPage } from './railNav'

export const useNavigation = defineStore('navigation', () => {
  const page = ref<AppPage>(DEFAULT_PAGE)

  function goTo(target: AppPage): void {
    page.value = target
  }

  return { page, goTo }
})
