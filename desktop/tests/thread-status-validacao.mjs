// Falhas cobertas: thread de card concluído continuando entre as ativas, "trabalhando" escondendo uma decisão pendente
// e card concluído sem conversa aparecendo como thread vazia.
// Execução: cd desktop && node --test tests/thread-status-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { threadStatus, splitThreads, showsAsThread } = jiti('../src/features/team/threadStatus.ts')

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

test('card concluído só vira thread se teve conversa', () => {
  assert.equal(showsAsThread({ column: 'Done', isTriaging: false, messageCount: 0 }), false)
  assert.equal(showsAsThread({ column: 'Done', isTriaging: false, messageCount: 3 }), true)
  assert.equal(showsAsThread({ column: 'In review', isTriaging: false, messageCount: 0 }), true)
  assert.equal(showsAsThread({ column: 'Backlog', isTriaging: true, messageCount: 0 }), true)
  assert.equal(showsAsThread({ column: 'Ready', isTriaging: false, messageCount: 5 }), false)
})
