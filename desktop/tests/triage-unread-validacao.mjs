// Validação do card item-126: canal #triagem e indicador de não lidas
// Execução: cd desktop && node tests/triage-unread-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { screenEntries } = jiti('../src/features/team/timeline.ts')
const { latestIncomingTime, pendingDecisionKeys, unreadKeyOf, threadUnreadKey } = jiti('../src/features/channels/unread.ts')

console.log('=== Iniciando Validação de Triagem e Não Lidas (item-126) ===\n')

let failed = 0

function test(name, fn) {
  try {
    fn()
    console.log(`[PASSOU] ${name}`)
  } catch (error) {
    failed += 1
    console.error(`[FALHOU] ${name}: ${error.message}`)
  }
}

const CHANNELS = [{ id: 7, name: 'dev', members: ['kairo', 'vexa'] }]
const TRIAGING = new Set(['item-1'])
const msg = (from, to, time, extra = {}) => ({ type: 'message', from, to, text: 't', time, projectId: 1, ...extra })
const incoming = (screen, feed) => latestIncomingTime(screenEntries(feed, screen, CHANNELS, undefined, TRIAGING))

const FEED = [
  msg('ember', 'user', 10),
  msg('kairo', 'vexa', 20, { channelId: 7 }),
  msg('axiom', 'sentry', 30, { itemId: 'item-1' }),
  msg('kairo', 'vexa', 40, { itemId: 'item-2' }),
  { type: 'task-result', projectId: 1, objective: 'o', concluded: true, turns: 1, agents: ['axiom'], time: 50, itemId: 'item-1' },
]

test('screenEntries triage mostra só cards em triagem', () => {
  const entries = screenEntries(FEED, { kind: 'triage' }, CHANNELS, undefined, TRIAGING)
  assert.deepEqual(entries.map((e) => e.time), [30, 50])
})

test('screenEntries triage sem cards em triagem fica vazio', () => {
  assert.equal(screenEntries(FEED, { kind: 'triage' }, CHANNELS).length, 0)
})

test('chaves de tela: geral, canal criado, triagem e thread', () => {
  assert.equal(unreadKeyOf({ kind: 'channel', channel: 'geral' }), 'geral')
  assert.equal(unreadKeyOf({ kind: 'channel', channel: 7 }), '7')
  assert.equal(unreadKeyOf({ kind: 'triage' }), 'triagem')
  assert.equal(unreadKeyOf({ kind: 'thread', itemId: 'item-2' }), threadUnreadKey('item-2'))
  assert.equal(unreadKeyOf({ kind: 'agent', agent: 'kairo' }), null)
})

test('mensagem não lida detectada em #geral, canal criado, thread e #triagem', () => {
  assert.equal(incoming({ kind: 'channel', channel: 'geral' }, FEED), 50)
  assert.equal(incoming({ kind: 'channel', channel: 7 }, FEED), 40)
  assert.equal(incoming({ kind: 'thread', itemId: 'item-2' }, FEED), 40)
  assert.equal(incoming({ kind: 'triage' }, FEED), 50)
})

test('mensagem do usuário não conta como não lida', () => {
  const feed = [msg('user', 'ember', 99), msg('user', 'kairo', 98, { itemId: 'item-2' })]
  assert.equal(incoming({ kind: 'channel', channel: 'geral' }, feed), null)
  assert.equal(incoming({ kind: 'thread', itemId: 'item-2' }, feed), null)
})

test('abrir a tela zera: lido no último horário recebido deixa de ser não lido', () => {
  const latest = incoming({ kind: 'channel', channel: 7 }, FEED)
  const lastRead = latest
  assert.ok(!(latest > lastRead))
  const next = incoming({ kind: 'channel', channel: 7 }, [...FEED, msg('kairo', 'vexa', 60, { channelId: 7 })])
  assert.ok(next > lastRead)
})

const ROUTING = {
  threadIds: new Set(['item-1', 'item-2']),
  triagingIds: TRIAGING,
  agentItems: { kairo: 'item-2' },
}

test('decisão de card em triagem fica vermelha na thread e em #triagem', () => {
  const keys = pendingDecisionKeys([{ itemId: 'item-1' }], [], ROUTING)
  assert.deepEqual([...keys].sort(), ['thread:item-1', 'triagem'])
})

test('decisão de card em andamento fica vermelha só na thread', () => {
  assert.deepEqual([...pendingDecisionKeys([{ itemId: 'item-2' }], [], ROUTING)], ['thread:item-2'])
})

test('decisão sem card cai em #geral', () => {
  assert.deepEqual([...pendingDecisionKeys([{}], [], ROUTING)], ['geral'])
  assert.deepEqual([...pendingDecisionKeys([{ itemId: 'item-9' }], [], ROUTING)], ['geral'])
})

test('permissão segue o card em que o agente trabalha', () => {
  assert.deepEqual([...pendingDecisionKeys([], [{ agent: 'kairo' }], ROUTING)], ['thread:item-2'])
  assert.deepEqual([...pendingDecisionKeys([], [{ agent: 'vexa' }], ROUTING)], ['geral'])
})

console.log(failed === 0 ? '\nTodos os testes passaram.' : `\n${failed} teste(s) falharam.`)
process.exit(failed === 0 ? 0 : 1)
