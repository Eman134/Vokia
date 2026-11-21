const FENCE = '```'
const CHECK_ITEM = /^\s*- \[( |x|X)\]/
const PERCENT = 100

export interface ChecklistProgress {
  done: number
  total: number
  percent: number
}

// Mesma regra de set_artifact_check no backend: marcadores "- [ ]"/"- [x]" fora de blocos cercados por crases.
export function checklistProgress(body: string): ChecklistProgress {
  let inFence = false
  let done = 0
  let total = 0
  for (const line of body.replace(/\r\n?/g, '\n').split('\n')) {
    if (line.trimStart().startsWith(FENCE)) {
      inFence = !inFence
      continue
    }
    const match = inFence ? null : CHECK_ITEM.exec(line)
    if (!match) continue
    total += 1
    if (match[1] !== ' ') done += 1
  }
  return { done, total, percent: total === 0 ? 0 : Math.round((done / total) * PERCENT) }
}
