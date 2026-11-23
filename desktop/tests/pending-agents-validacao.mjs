// Validação do card item-9: filtragem por escopo e agrupamento por estado de agentes pendentes
// Execução: cd desktop && node tests/pending-agents-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)

const {
  isPendingState,
  selectPendingAgents,
  isAgentInScreen,
  filterPendingAgentsForScreen,
  groupPendingAgentsByState,
  formatPendingStatusLabel,
} = jiti('../src/features/team/pendingAgents.ts')

console.log('=== Iniciando Validação de pendingAgents (item-9) ===\n')

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

function agent(id, name) {
  return { id, name, role: 'Dev', kind: 'specialist' }
}

const ROSTER = [
  agent('ember', 'Ember'),
  agent('kairo', 'Kairo'),
  agent('vexa', 'Vexa'),
  agent('sentry', 'Sentry'),
]

const CHANNELS = [
  { id: 1, name: 'backend', members: ['ember', 'kairo'] },
  { id: 2, name: 'frontend', members: ['ember', 'vexa'] },
]

test('isPendingState e selectPendingAgents filtram apenas thinking e sending', () => {
  assert.equal(isPendingState('thinking'), true)
  assert.equal(isPendingState('sending'), true)
  assert.equal(isPendingState('waiting'), false)
  assert.equal(isPendingState('idle'), false)

  const states = {
    ember: 'idle',
    kairo: 'thinking',
    vexa: 'sending',
    sentry: 'waiting',
  }
  const pending = selectPendingAgents(ROSTER, states)
  assert.deepEqual(
    pending.map((a) => a.id),
    ['kairo', 'vexa'],
  )
})

test('filterPendingAgentsForScreen respeita escopo all e agent', () => {
  const pending = [ROSTER[1], ROSTER[2]]
  assert.deepEqual(
    filterPendingAgentsForScreen(pending, { kind: 'all' }, { channels: CHANNELS }).map((a) => a.id),
    ['kairo', 'vexa'],
  )
  assert.deepEqual(
    filterPendingAgentsForScreen(pending, { kind: 'agent', agent: 'vexa' }, { channels: CHANNELS }).map((a) => a.id),
    ['vexa'],
  )
  assert.deepEqual(
    filterPendingAgentsForScreen(pending, { kind: 'agent', agent: 'sentry' }, { channels: CHANNELS }),
    [],
  )
})

test('filterPendingAgentsForScreen respeita canal #geral e canais específicos', () => {
  const pending = [ROSTER[1], ROSTER[2], ROSTER[3]]
  assert.deepEqual(
    filterPendingAgentsForScreen(pending, { kind: 'channel', channel: 'geral' }, { channels: CHANNELS }).map((a) => a.id),
    ['kairo', 'vexa', 'sentry'],
  )
  assert.deepEqual(
    filterPendingAgentsForScreen(pending, { kind: 'channel', channel: 1 }, { channels: CHANNELS }).map((a) => a.id),
    ['kairo'],
  )
  assert.deepEqual(
    filterPendingAgentsForScreen(pending, { kind: 'channel', channel: 999 }, { channels: CHANNELS }),
    [],
  )
})

test('filterPendingAgentsForScreen respeita escopo thread (assignee e participants)', () => {
  const pending = [ROSTER[1], ROSTER[2], ROSTER[3]]
  const thread = { assignee: 'vexa', participants: ['kairo'] }
  assert.deepEqual(
    filterPendingAgentsForScreen(pending, { kind: 'thread', itemId: 'item-9' }, { channels: CHANNELS, thread }).map(
      (a) => a.id,
    ),
    ['kairo', 'vexa'],
  )
  assert.equal(isAgentInScreen('sentry', { kind: 'thread', itemId: 'item-9' }, { channels: CHANNELS, thread }), false)
  assert.deepEqual(
    filterPendingAgentsForScreen(pending, { kind: 'thread', itemId: 'item-9' }, { channels: CHANNELS }),
    [],
  )
})

test('filterPendingAgentsForScreen usa agentItems para isolar estados por task sem vazar outras tasks', () => {
  const pending = [ROSTER[1], ROSTER[2], ROSTER[3]]
  const thread = { assignee: 'vexa', participants: ['kairo', 'vexa'] }
  const agentItems = {
    kairo: 'item-16',
    vexa: 'item-15',
    sentry: undefined,
  }
  assert.deepEqual(
    filterPendingAgentsForScreen(
      pending,
      { kind: 'thread', itemId: 'item-16' },
      { channels: CHANNELS, thread, agentItems },
    ).map((a) => a.id),
    ['kairo'],
  )
  assert.deepEqual(
    filterPendingAgentsForScreen(
      pending,
      { kind: 'thread', itemId: 'item-15' },
      { channels: CHANNELS, thread, agentItems },
    ).map((a) => a.id),
    ['vexa'],
  )
  assert.equal(
    isAgentInScreen('sentry', { kind: 'thread', itemId: 'item-16' }, { channels: CHANNELS, thread, agentItems }),
    false,
  )
})

test('groupPendingAgentsByState agrupa por estado preservando thinking antes de sending e stateSince por agente', () => {
  const states = {
    ember: 'sending',
    kairo: 'thinking',
    vexa: 'thinking',
    sentry: 'sending',
  }
  const stateSince = {
    ember: 1000,
    kairo: 2000,
    vexa: 3000,
    sentry: 4000,
  }
  const groups = groupPendingAgentsByState(ROSTER, states, stateSince)
  assert.equal(groups.length, 2)
  assert.equal(groups[0].state, 'thinking')
  assert.deepEqual(
    groups[0].entries.map((entry) => ({ id: entry.agent.id, since: entry.since })),
    [
      { id: 'kairo', since: 2000 },
      { id: 'vexa', since: 3000 },
    ],
  )
  assert.equal(groups[1].state, 'sending')
  assert.deepEqual(
    groups[1].entries.map((entry) => ({ id: entry.agent.id, since: entry.since })),
    [
      { id: 'ember', since: 1000 },
      { id: 'sentry', since: 4000 },
    ],
  )
})

test('formatPendingStatusLabel formata singular, plural e múltiplos grupos', () => {
  assert.equal(formatPendingStatusLabel([]), undefined)

  const single = groupPendingAgentsByState([ROSTER[1]], { kairo: 'thinking' })
  assert.equal(formatPendingStatusLabel(single), 'Kairo está pensando')

  const sameState = groupPendingAgentsByState([ROSTER[1], ROSTER[2]], { kairo: 'thinking', vexa: 'thinking' })
  assert.equal(formatPendingStatusLabel(sameState), 'Kairo, Vexa estão pensando')

  const multiGroup = groupPendingAgentsByState(
    [ROSTER[1], ROSTER[2], ROSTER[3]],
    { kairo: 'thinking', vexa: 'thinking', sentry: 'sending' },
  )
  assert.equal(formatPendingStatusLabel(multiGroup), 'Kairo, Vexa estão pensando; Sentry está enviando')
})

const failed = results.filter((r) => !r.passed)
console.log(`\n=== Resumo: ${results.length - failed.length}/${results.length} passaram ===`)
if (failed.length > 0) {
  process.exit(1)
}
