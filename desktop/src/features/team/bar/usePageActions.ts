import { computed, onMounted, onUnmounted, type Ref } from 'vue'
import { ACTION_SHORTCUT_MODIFIER, PAGE_ACTIONS, type ComposeSize, type PageAction } from '../pageActions'
import type { RailPage } from '../../../shell/railNav'
import { useNavigation } from '../../../shell/useNavigation'

const ACTION_SHORTCUT_CODES = ['Digit1', 'Digit2']

export function usePageActions(actionMenuOpen: Ref<boolean>, onCompose: (size: ComposeSize) => void) {
  const navigation = useNavigation()
  const railPage = computed<RailPage | null>(() =>
    navigation.page === 'novo-projeto' || navigation.page === 'onboarding' ? null : navigation.page,
  )
  const pageActions = computed<readonly PageAction[]>(() =>
    railPage.value === null ? [] : PAGE_ACTIONS[railPage.value],
  )

  function runAction(action: PageAction): void {
    actionMenuOpen.value = false
    if (action.size !== undefined) {
      onCompose(action.size)
    }
  }

  function onShortcut(event: KeyboardEvent): void {
    const index = ACTION_SHORTCUT_CODES.indexOf(event.code)
    const action = index >= 0 ? pageActions.value[index] : undefined
    if (action === undefined || !event.getModifierState(ACTION_SHORTCUT_MODIFIER)) {
      return
    }
    event.preventDefault()
    runAction(action)
  }

  onMounted(() => window.addEventListener('keydown', onShortcut))
  onUnmounted(() => window.removeEventListener('keydown', onShortcut))

  return { railPage, pageActions, runAction }
}
