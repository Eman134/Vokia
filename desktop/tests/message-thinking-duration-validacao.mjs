import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { formatThinkingDuration } = jiti('../src/features/team/messageUsageFormat.ts')

test('formatThinkingDuration — retorna null para valores ausentes ou inválidos', () => {
  assert.equal(formatThinkingDuration(undefined), null)
  assert.equal(formatThinkingDuration(-100), null)
  assert.equal(formatThinkingDuration(Number.NaN), null)
})

test('formatThinkingDuration — formata segundos com mínimo de 1s', () => {
  assert.equal(formatThinkingDuration(0), '1s')
  assert.equal(formatThinkingDuration(450), '1s')
  assert.equal(formatThinkingDuration(12_900), '12s')
  assert.equal(formatThinkingDuration(59_999), '59s')
})

test('formatThinkingDuration — formata minutos e segundos a partir de 60s', () => {
  assert.equal(formatThinkingDuration(60_000), '1m')
  assert.equal(formatThinkingDuration(72_000), '1m 12s')
  assert.equal(formatThinkingDuration(125_400), '2m 5s')
})
