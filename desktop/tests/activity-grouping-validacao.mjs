import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { activityRows, activityClauses } = jiti('../src/features/team/activitySummary.ts')

let seq = 0
const tool = (over = {}) => ({ callId: `c${seq++}`, name: 'mcp__vokia__backlog_set_responsible', category: 'mcp', target: '', status: 'failed', ...over })
const rows = (tools) => activityRows(tools, () => undefined, () => undefined)

test('falhas consecutivas idênticas viram uma linha com contagem', () => {
  const tools = Array.from({ length: 24 }, () => tool())
  const result = rows(tools)
  assert.equal(result.length, 1)
  assert.equal(result[0].count, 24)
  assert.equal(result[0].verb, 'Tentou usar')
  assert.equal(result[0].id, tools[0].callId)
})

test('contador do resumo segue contando todas as chamadas', () => {
  assert.deepEqual(activityClauses(Array.from({ length: 24 }, () => tool())), ['usou 24 integrações'])
})

test('não agrupa status diferentes nem entradas não consecutivas', () => {
  const result = rows([tool(), tool({ status: 'ok' }), tool(), tool({ name: 'mcp__vokia__other' }), tool()])
  assert.equal(result.length, 5)
  assert.ok(result.every((row) => row.count === undefined))
})

test('linha única não ganha contagem', () => {
  assert.equal(rows([tool()])[0].count, undefined)
})
