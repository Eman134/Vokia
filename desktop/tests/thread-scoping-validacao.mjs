// Validação do card item-16: isolamento de mensagens e participantes por thread
// Execução: cd desktop && node tests/thread-scoping-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { screenEntries } = jiti('../src/features/team/timeline.ts')

console.log('=== Iniciando Validação de Thread Scoping (item-16) ===\n')

const results = []

function test(name, fn) {
  try {
    fn()
    results.push({ name, passed: true })
    console.log(`[PASSOU] ${name}`)
  } catch (error) {
    results.push({ name, passed: false, error: error.message })
    console.error(`[FALHOU] ${name}: ${error.message}`)
  }
}

const CHANNELS = [{ id: 1, name: 'dev', members: ['ember', 'kairo', 'vexa'] }]

test('screenEntries na thread exibe mensagens diretas e delegadas da task sem vazar outras tasks', () => {
  const feed = [
    { type: 'message', from: 'user', to: 'kairo', text: 'msg 16', time: 1, projectId: 1, itemId: 'item-16' },
    { type: 'message', from: 'kairo', to: 'vexa', text: 'delegado 16', time: 2, projectId: 1, itemId: 'item-16' },
    { type: 'message', from: 'vexa', to: 'sentry', text: 'outra task 15', time: 3, projectId: 1, itemId: 'item-15' },
    { type: 'message', from: 'kairo', to: 'user', text: 'geral', time: 4, projectId: 1 },
    { type: 'permission-result', agent: 'vexa', detail: 'bash', outcome: 'allowed', time: 5 },
    { type: 'permission-result', agent: 'sentry', detail: 'bash', outcome: 'allowed', time: 6 },
    { type: 'task-result', projectId: 1, objective: 'o16', concluded: true, turns: 2, agents: ['kairo', 'vexa'], time: 7, itemId: 'item-16' },
    { type: 'task-result', projectId: 1, objective: 'o15', concluded: true, turns: 1, agents: ['sentry'], time: 8, itemId: 'item-15' },
  ]

  const entries = screenEntries(feed, { kind: 'thread', itemId: 'item-16' }, CHANNELS)
  assert.deepEqual(
    entries.map((e) => e.time),
    [1, 2, 5, 7],
  )
})

test('screenEntries na thread inclui permission-result de agente ativo via threadAgents antes de enviar mensagem', () => {
  const feed = [
    { type: 'permission-result', agent: 'sentry', detail: 'read', outcome: 'allowed', time: 10 },
    { type: 'permission-result', agent: 'ember', detail: 'read', outcome: 'allowed', time: 11 },
  ]
  const entries = screenEntries(feed, { kind: 'thread', itemId: 'item-16' }, CHANNELS, new Set(['sentry']))
  assert.deepEqual(
    entries.map((e) => e.time),
    [10],
  )
})

const failed = results.filter((r) => !r.passed)
console.log(`\n=== Resumo: ${results.length - failed.length}/${results.length} passaram ===`)
if (failed.length > 0) {
  process.exit(1)
}
