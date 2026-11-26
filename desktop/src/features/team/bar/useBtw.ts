import { reactive } from 'vue'
import { askBtw } from '../teamApi'

export interface BtwSession {
  id: number
  agent: string
  question: string
  status: 'thinking' | 'done' | 'error'
  answer?: string
  error?: string
}

const state = reactive<{ active: BtwSession | null; nextId: number }>({
  active: null,
  nextId: 1,
})

export function useBtw() {
  async function ask(args: { projectId: number | null; agent: string | null; question: string }): Promise<void> {
    const id = state.nextId++
    const targetAgent = args.agent ?? 'ember'
    state.active = {
      id,
      agent: targetAgent,
      question: args.question,
      status: 'thinking',
    }
    try {
      const res = await askBtw(args)
      if (state.active?.id !== id) return
      state.active = {
        id,
        agent: res.agent,
        question: args.question,
        status: 'done',
        answer: res.answer,
      }
    } catch (err) {
      if (state.active?.id !== id) return
      state.active = {
        id,
        agent: targetAgent,
        question: args.question,
        status: 'error',
        error: err instanceof Error ? err.message : String(err),
      }
    }
  }

  function dismiss(): void {
    state.active = null
  }

  return {
    state,
    ask,
    dismiss,
  }
}
