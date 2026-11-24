// O backend grava a atividade como "dd/mm/aaaa, HH:MM:SS" (ACTIVITY_TIME_FORMAT em db/backlog.rs).
const STORED_FORMAT = /^(\d{2})\/(\d{2})\/(\d{4}), (\d{2}):(\d{2})/
const TODAY = 'hoje'
const YESTERDAY = 'ontem'

function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
}

export function activityTime(at: string, now: Date = new Date()): string {
  const match = STORED_FORMAT.exec(at)
  if (!match) return at
  const [, day, month, year, hour, minute] = match
  const when = new Date(Number(year), Number(month) - 1, Number(day))
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1)
  const clock = `${hour}:${minute}`
  if (sameDay(when, now)) return `${TODAY}, ${clock}`
  if (sameDay(when, yesterday)) return `${YESTERDAY}, ${clock}`
  if (when.getFullYear() === now.getFullYear()) return `${day}/${month}, ${clock}`
  return `${day}/${month}/${year}, ${clock}`
}
