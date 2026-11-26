import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { itemUsageIn, activeThinkingMsForItem, formatItemUsage } = jiti('../src/features/team/itemUsage.ts')

const msg = (itemId, costUsd, thinkingMs, { durationMs, tools = [] } = {}) => ({
  type: 'message', from: 'user', to: 'user', text: '', time: 0, projectId: 1, itemId,
  usage: costUsd === undefined && durationMs === undefined ? undefined : { costUsd, durationMs },
  activity: thinkingMs === undefined && tools.length === 0 ? undefined : { thinkingMs, tools },
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

test('formatItemUsage monta custo e tempo, sem 1s falso quando workMs === 0', () => {
  assert.equal(formatItemUsage({ costUsd: 0.75, workMs: 65_000, turns: 2 }), '$0.75 · 1m 5s')
  assert.equal(formatItemUsage({ costUsd: 0, workMs: 12_000, turns: 1 }), '12s')
  assert.equal(formatItemUsage({ costUsd: 0.5, workMs: 0, turns: 1 }), '$0.50')
  assert.equal(formatItemUsage({ costUsd: 0, workMs: 0, turns: 1 }), null)
  assert.equal(formatItemUsage({ costUsd: 0, workMs: 0, turns: 0 }), null)
})

test('itemUsageIn usa durationMs de TurnUsage e faz fallback legado para thinkingMs + tools[].durationMs', () => {
  const feed = [
    msg('item-1', 0.1, 2_000, { durationMs: 8_500, tools: [{ name: 'read', durationMs: 1_000 }] }),
    msg('item-1', 0.2, 3_000, { tools: [{ name: 'bash', durationMs: 1_500 }, { name: 'edit', durationMs: 500 }] }),
    msg('item-1', undefined, undefined, { durationMs: 4_000 }),
  ]
  assert.deepEqual(itemUsageIn(feed, 'item-1'), { costUsd: 0.30000000000000004, workMs: 17_500, turns: 3 })
})

test('activeThinkingMsForItem soma múltiplos agentes e réplicas em paralelo no mesmo card, mesmo com turns === 0', () => {
  const now = 100_000
  const activeMs = activeThinkingMsForItem({
    itemId: 'item-402',
    projectId: 1,
    states: { axiom: 'thinking', vexa: 'thinking', 'kairo#2': 'thinking' },
    stateSince: { axiom: 90_000, vexa: 95_500, 'kairo#2': 98_000 },
    stateItem: { axiom: 'item-402', vexa: 'item-402', 'kairo#2': 'item-402' },
    lastProject: { axiom: 1, vexa: 1, 'kairo#2': 1 },
    now,
  })
  assert.equal(activeMs, 16_500)
  assert.equal(formatItemUsage({ costUsd: 0, workMs: activeMs, turns: 0 }), '16s')
})

test('activeThinkingMsForItem isola por projectId e exclui estados sending, waiting e idle', () => {
  const now = 50_000
  const activeMs = activeThinkingMsForItem({
    itemId: 'item-402',
    projectId: 1,
    states: {
      axiom: 'thinking',
      vexa: 'thinking',
      kairo: 'sending',
      sentry: 'waiting',
      ember: 'idle',
    },
    stateSince: { axiom: 40_000, vexa: 30_000, kairo: 10_000, sentry: 10_000, ember: 10_000 },
    stateItem: { axiom: 'item-402', vexa: 'item-402', kairo: 'item-402', sentry: 'item-402', ember: 'item-402' },
    lastProject: { axiom: 1, vexa: 2, kairo: 1, sentry: 1, ember: 1 },
    now,
  })
  assert.equal(activeMs, 10_000)
  assert.equal(
    activeThinkingMsForItem({
      itemId: 'item-402',
      projectId: null,
      states: { axiom: 'thinking' },
      stateSince: { axiom: 40_000 },
      stateItem: { axiom: 'item-402' },
      lastProject: { axiom: 1 },
      now,
    }),
    0,
  )
})
