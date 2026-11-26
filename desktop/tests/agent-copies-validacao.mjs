// Validação do card item-11: cópias de agentes e separação de conversa por instância
// Execução: cd desktop && node tests/agent-copies-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)

const { replicaOf, worktreeFolder } = jiti('../src/features/team/agentCopies.ts')
const { screenEntries } = jiti('../src/features/team/timeline.ts')
const { screenAgent, screenDestination, screenPlaceholder } = jiti('../src/features/team/screenRecipient.ts')
const { isAgentInScreen, filterPendingAgentsForScreen } = jiti('../src/features/team/pendingAgents.ts')

console.log('=== Iniciando Validação de Cópias de Agentes (item-11) ===\n')

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

const COPY_SCREEN = {
  kind: 'copy',
  copyId: 'vexa-2',
  baseAgent: 'vexa',
  number: 2,
  itemId: 'item-11',
  itemTitle: 'Melhorar visual das cópias',
}

test('replicaOf e worktreeFolder extraem base, número e pasta', () => {
  assert.deepEqual(replicaOf('vexa-2'), { base: 'vexa', number: 2 })
  assert.deepEqual(replicaOf('kairo-10'), { base: 'kairo', number: 10 })
  assert.equal(replicaOf('vexa'), null)
  assert.equal(worktreeFolder('C:\\Users\\Kayke\\worktrees\\1\\item-11'), 'item-11')
  assert.equal(worktreeFolder('/tmp/wt/pix-qr/'), 'pix-qr')
})

test('screenEntries isola mensagens, permissões e resultados por instância da cópia', () => {
  const feed = [
    { type: 'message', from: 'user', to: 'vexa', text: 'Oi Vexa base', time: 1, projectId: 1 },
    { type: 'message', from: 'user', to: 'vexa-2', text: 'Oi cópia 2', time: 2, projectId: 1 },
    { type: 'message', from: 'vexa-2', to: 'user', text: 'Resposta cópia 2', time: 3, projectId: 1 },
    { type: 'message', from: 'user', to: 'vexa-3', text: 'Oi cópia 3', time: 4, projectId: 1 },
    { type: 'message', from: 'user', to: 'vexa-2', text: 'Em canal', time: 5, projectId: 1, channelId: 9 },
    { type: 'permission-result', agent: 'vexa-2', detail: 'npm test', outcome: 'allowed', time: 6 },
    { type: 'permission-result', agent: 'vexa', detail: 'git status', outcome: 'allowed', time: 7 },
    { type: 'task-result', projectId: 1, objective: 'Card', concluded: true, turns: 1, agents: ['vexa-2'], time: 8 },
  ]
  const channels = [{ id: 9, name: 'front', members: ['vexa'] }]

  const copyFeed = screenEntries(feed, COPY_SCREEN, channels)
  assert.deepEqual(
    copyFeed.map((e) => e.time),
    [2, 3, 6, 8],
  )

  const baseFeed = screenEntries(feed, { kind: 'agent', agent: 'vexa' }, channels)
  assert.deepEqual(
    baseFeed.map((e) => e.time),
    [1, 7],
  )
})

test('screenRecipient direciona mensagens e placeholder para copyId', () => {
  assert.equal(screenAgent('conversas', COPY_SCREEN), 'vexa-2')
  assert.equal(screenAgent('backlog', COPY_SCREEN), null)

  assert.deepEqual(screenDestination('conversas', COPY_SCREEN, [], ['ember', 'vexa'], null), {
    to: 'vexa-2',
    channelId: null,
  })
  assert.deepEqual(screenDestination('conversas', COPY_SCREEN, [], ['ember', 'vexa'], 'kairo'), {
    to: 'kairo',
    channelId: null,
  })

  const nameOf = (id) => (id === 'vexa' ? 'Vexa' : id)
  assert.equal(screenPlaceholder('conversas', COPY_SCREEN, nameOf), 'Mensagem para Vexa 2')
})

test('pendingAgents filtra estado apenas da instância selecionada', () => {
  assert.equal(isAgentInScreen('vexa-2', COPY_SCREEN, { channels: [] }), true)
  assert.equal(isAgentInScreen('vexa', COPY_SCREEN, { channels: [] }), false)
  assert.equal(isAgentInScreen('vexa-2', { kind: 'agent', agent: 'vexa' }, { channels: [] }), false)

  const pending = [
    { id: 'vexa', name: 'Vexa', role: 'Frontend', kind: 'specialist' },
    { id: 'vexa-2', name: 'Vexa 2', role: 'Frontend', kind: 'specialist' },
  ]
  assert.deepEqual(
    filterPendingAgentsForScreen(pending, COPY_SCREEN, { channels: [] }).map((a) => a.id),
    ['vexa-2'],
  )
})

const failed = results.filter((r) => !r.passed).length
console.log(`\nResumo: ${results.length - failed}/${results.length} testes passaram.`)
if (failed > 0) process.exit(1)
