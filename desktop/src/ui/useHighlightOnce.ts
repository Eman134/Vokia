const highlightedKeys = new Set<string>()

export function useHighlightOnce(key: string): boolean {
  if (highlightedKeys.has(key)) return false
  highlightedKeys.add(key)
  return true
}
