import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { liveTimeline } = jiti('../src/features/console/liveTimeline.ts')

let seq = 0
const entry = (kind, text = '') => ({ seq: ++seq, kind, text, time: seq })
const tool = (callId, name = 'Read') => ({
  callId,
  name,
  category: 'read',
  target: `${callId}.ts`,
  status: 'ok',
})

test('intercala texto, ferramentas e texto na ordem do turno atual', () => {
  const entries = [
    entry('start'),
    entry('text', 'Texto antigo'),
    entry('tool', 'Read old.ts'),
    entry('start'),
    entry('text', 'Primeiro trecho'),
    entry('text', 'Continuação'),
    entry('tool', 'Read a.ts'),
    entry('tool', 'Read b.ts'),
    entry('text', 'Depois das ferramentas'),
  ]
  const t1 = tool('t1')
  const t2 = tool('t2')

  assert.deepEqual(liveTimeline(entries, [t1, t2]), [
    { kind: 'text', text: 'Primeiro trecho\nContinuação' },
    { kind: 'tools', tools: [t1, t2] },
    { kind: 'text', text: 'Depois das ferramentas' },
  ])
})

test('coloca ferramentas excedentes ao final quando faltam linhas tool no console', () => {
  const entries = [entry('start'), entry('tool', 'Read a.ts'), entry('text', 'Meio')]
  const t1 = tool('t1')
  const t2 = tool('t2')
  const t3 = tool('t3')

  assert.deepEqual(liveTimeline(entries, [t1, t2, t3]), [
    { kind: 'tools', tools: [t1] },
    { kind: 'text', text: 'Meio' },
    { kind: 'tools', tools: [t2, t3] },
  ])
})

test('retorna apenas ferramentas quando não há texto no turno', () => {
  const entries = [entry('start'), entry('tool', 'Read a.ts'), entry('tool', 'Read b.ts')]
  const t1 = tool('t1')
  const t2 = tool('t2')

  assert.deepEqual(liveTimeline(entries, [t1, t2]), [{ kind: 'tools', tools: [t1, t2] }])
})

test('lida com console vazio com e sem ferramentas', () => {
  const t1 = tool('t1')
  assert.deepEqual(liveTimeline([], []), [])
  assert.deepEqual(liveTimeline([], [t1]), [{ kind: 'tools', tools: [t1] }])
})
