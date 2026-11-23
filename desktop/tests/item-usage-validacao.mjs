import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { itemUsageIn } = jiti('../src/features/team/itemUsage.ts')

const msg = (itemId, costUsd, thinkingMs) => ({
  type: 'message', from: 'user', to: 'user', text: '', time: 0, projectId: 1, itemId,
  usage: costUsd === undefined ? undefined : { costUsd },
  activity: thinkingMs === undefined ? undefined : { thinkingMs, tools: [] },
})

test('soma custo e tempo de todos os agentes do item', () => {
  const feed = [msg('item-1', 0.5, 10_000), msg('item-2', 9, 9_000), msg('item-1', 0.25, 5_000), { type: 'notice', text: '', time: 0 }]
  assert.deepEqual(itemUsageIn(feed, 'item-1'), { costUsd: 0.75, workMs: 15_000, turns: 2 })
})

test('ignora mensagens sem uso e valores inválidos; item sem dados fica zerado', () => {
  const feed = [msg('item-1', undefined, undefined), msg('item-1', Number.NaN, -5)]
  assert.deepEqual(itemUsageIn(feed, 'item-1'), { costUsd: 0, workMs: 0, turns: 0 })
  assert.deepEqual(itemUsageIn([], 'x'), { costUsd: 0, workMs: 0, turns: 0 })
})

test('atualiza ao entrar nova mensagem (função pura sobre o feed atual)', () => {
  const feed = [msg('item-1', 1, 1000)]
  assert.equal(itemUsageIn(feed, 'item-1').turns, 1)
  feed.push(msg('item-1', 1, 1000))
  assert.equal(itemUsageIn(feed, 'item-1').costUsd, 2)
})

test('formatItemUsage monta custo e tempo, ou nada sem turnos', () => {
  const { formatItemUsage } = jiti('../src/features/team/itemUsage.ts')
  assert.equal(formatItemUsage({ costUsd: 0.75, workMs: 65_000, turns: 2 }), '$0.75 · 1m 5s')
  assert.equal(formatItemUsage({ costUsd: 0, workMs: 12_000, turns: 1 }), '12s')
  assert.equal(formatItemUsage({ costUsd: 0, workMs: 0, turns: 0 }), null)
})
