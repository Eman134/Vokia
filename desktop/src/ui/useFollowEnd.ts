import { nextTick, ref, type Ref } from 'vue'

const AT_END_TOLERANCE_PX = 4

export function useFollowEnd(scroller: Ref<HTMLElement | null>) {
  // Só o próprio scroll decide: arrastar a barra ou rolar com a roda sai do fim e volta ao chegar nele.
  const followingEnd = ref(true)

  function onScroll(): void {
    const element = scroller.value
    if (element === null) return
    followingEnd.value = element.scrollHeight - element.scrollTop - element.clientHeight <= AT_END_TOLERANCE_PX
  }

  async function followEnd(): Promise<void> {
    await nextTick()
    const element = scroller.value
    if (followingEnd.value && element !== null) {
      element.scrollTop = element.scrollHeight
    }
  }

  async function jumpToEnd(): Promise<void> {
    followingEnd.value = true
    await followEnd()
  }

  return { followingEnd, onScroll, followEnd, jumpToEnd }
}
