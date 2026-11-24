import type { AppPage } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import { useBacklog } from '../backlog/useBacklog'

const BACKLOG_PAGE: AppPage = 'backlog'

// O modal do card mora na página Backlog: abrir sem navegar não mostraria nada.
export function useOpenCard(): (itemId: string) => void {
  const backlog = useBacklog()
  const navigation = useNavigation()
  return (itemId) => {
    backlog.open(itemId)
    navigation.goTo(BACKLOG_PAGE)
  }
}
