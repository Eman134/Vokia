// Falhas cobertas: frase do agente perdida na linha, comando escondido quando há frase,
// linha vazia sem frase e repetições com comandos diferentes fundidas numa só.
// Execução: cd desktop && node --test tests/command-description-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { activityRows, activitySentence } = jiti('../src/features/team/activitySummary.ts')

const bash = (callId, target, description, status = 'ok') => ({ callId, name: 'Bash', category: 'command', target, status, description })
const rows = (tools) => activityRows(tools, () => undefined, () => undefined)

test('com frase, a linha mostra a frase e o comando embaixo', () => {
  const [row] = rows([bash('t1', 'sed -n 205,216p plano.md', 'Mostra os limites da seção')])
  assert.equal(row.verb, 'Mostra os limites da seção')
  assert.equal(row.command, 'sed -n 205,216p plano.md')
  assert.equal(row.chip, undefined)
})

test('sem frase, a linha continua como antes', () => {
  const [row] = rows([bash('t1', 'git status')])
  assert.equal(row.verb, 'Executou')
  assert.equal(row.chip.label, 'git status')
  assert.equal(row.command, undefined)
})

test('falha continua marcada', () => {
  const [row] = rows([bash('t1', 'npm test', 'Roda os testes', 'failed')])
  assert.equal(row.failed, true)
  assert.equal(row.verb, 'Roda os testes')
})

test('mesma frase com comandos diferentes não se funde', () => {
  assert.equal(rows([bash('t1', 'ls a', 'Lista a pasta'), bash('t2', 'ls b', 'Lista a pasta')]).length, 2)
  assert.equal(rows([bash('t1', 'ls a', 'Lista a pasta'), bash('t2', 'ls a', 'Lista a pasta')]).length, 1)
})

test('resumo de um comando só usa a frase', () => {
  assert.equal(activitySentence([bash('t1', 'git status', 'Mostra o estado')]), 'Mostra o estado')
  assert.equal(activitySentence([bash('t1', 'git status')]), 'git status')
})
