// Falhas cobertas: thread de card concluído continuando entre as ativas, "trabalhando" escondendo uma decisão pendente
// e card concluído sem conversa aparecendo como thread vazia.
// Execução: cd desktop && node --test tests/thread-status-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { threadStatus, splitThreads, showsAsThread, categorizeThreads, DONE_THREAD_RETENTION_MS } = jiti('../src/features/team/threadStatus.ts')

const base = { column: 'In progress', isTriaging: false, hasOpenQuestion: false, hasPendingPermission: false, hasWorkingAgent: false }

test('estado da thread: concluída > precisa de você > trabalhando > parada', () => {
  assert.equal(threadStatus({ ...base, column: 'Done', hasOpenQuestion: true }), 'done')
  assert.equal(threadStatus({ ...base, hasOpenQuestion: true, hasWorkingAgent: true }), 'needs-you')
  assert.equal(threadStatus({ ...base, hasPendingPermission: true }), 'needs-you')
  assert.equal(threadStatus({ ...base, hasWorkingAgent: true }), 'working')
  assert.equal(threadStatus({ ...base, isTriaging: true }), 'working')
  assert.equal(threadStatus(base), 'idle')
})

test('divide em ativas e concluídas, mantendo a ordem de cada lista', () => {
  const threads = [{ itemId: 'a', status: 'idle' }, { itemId: 'b', status: 'done' }, { itemId: 'c', status: 'working' }, { itemId: 'd', status: 'done' }]
  const { active, done } = splitThreads(threads)
  assert.deepEqual(active.map((thread) => thread.itemId), ['a', 'c'])
  assert.deepEqual(done.map((thread) => thread.itemId), ['b', 'd'])
})

test('triagem fica fora das threads; Ready aparece e Done arquiva após o tempo', () => {
  const now = 1_700_000_000_000
  assert.equal(showsAsThread({ column: 'Done', isTriaging: false, messageCount: 0 }, now), false)
  assert.equal(showsAsThread({ column: 'Done', isTriaging: false, messageCount: 3, lastActivity: now - 1000 }, now), true)
  assert.equal(showsAsThread({ column: 'Done', isTriaging: false, messageCount: 3, lastActivity: now - DONE_THREAD_RETENTION_MS - 1 }, now), false)
  assert.equal(showsAsThread({ column: 'In review', isTriaging: false, messageCount: 0 }, now), true)
  assert.equal(showsAsThread({ column: 'In progress', isTriaging: true, messageCount: 0 }, now), true)
  assert.equal(showsAsThread({ column: 'Backlog', isTriaging: true, messageCount: 0 }, now), false)
  assert.equal(showsAsThread({ column: 'Ready', isTriaging: false, messageCount: 0 }, now), true)
})

test('categorias de threads: recentes primeiro, depois em andamento, sem triagem e concluídas por último', () => {
  const categories = categorizeThreads([
    { status: 'done', column: 'Done', lastActivity: 300 },
    { status: 'idle', column: 'Ready', lastActivity: 200 },
    { status: 'working', column: 'In progress', lastActivity: 500 },
  ])
  assert.deepEqual(
    categories.map((c) => c.key),
    ['recent', 'in-progress', 'done'],
  )
})
