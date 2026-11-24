// Falhas cobertas: aprovações automáticas em linhas soltas, aprovação do usuário ou negação escondidas no grupo,
// agentes diferentes misturados, mensagem no meio sem separar os grupos e frase com comandos repetidos ou longa demais.
// Execução: cd desktop && node --test tests/auto-permission-group-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { buildTimeline } = jiti('../src/features/team/timeline.ts')
const { autoPermissionSentence } = jiti('../src/features/team/permissionSummary.ts')

let clock = 0
const permission = (outcome, detail = 'git status', agent = 'axiom') => ({ type: 'permission-result', agent, detail, outcome, time: ++clock })
const reply = { type: 'message', from: 'axiom', to: 'user', text: 'As duas tasks estão finalizadas', time: ++clock }
const kinds = (timeline) => timeline.map((item) => (item.type === 'permissions' ? `grupo:${item.results.length}` : item.type === 'entry' ? item.entry.type : item.type))

test('aprovações automáticas seguidas do mesmo agente viram um grupo antes da resposta', () => {
  const feed = [...Array.from({ length: 7 }, () => permission('bypass')), reply]
  assert.deepEqual(kinds(buildTimeline(feed)), ['grupo:7', 'message'])
})

test('modo auto e regra do projeto entram no grupo; aprovação do usuário e negação ficam à vista e separam', () => {
  const feed = [permission('auto'), permission('project-rule'), permission('allowed'), permission('bypass'), permission('denied'), permission('bypass')]
  assert.deepEqual(kinds(buildTimeline(feed)), ['grupo:2', 'permission-result', 'grupo:1', 'permission-result', 'grupo:1'])
})

test('agentes diferentes e mensagem no meio não juntam grupos; erro oculto não separa', () => {
  const error = { type: 'error', message: 'falha', time: ++clock }
  const feed = [permission('bypass'), permission('bypass', 'ls', 'vexa'), permission('bypass'), reply, permission('bypass'), error, permission('bypass')]
  assert.deepEqual(kinds(buildTimeline(feed)), ['grupo:1', 'grupo:1', 'grupo:1', 'message', 'grupo:2'])
})

test('a frase resume os comandos sem repetir e corta nos três primeiros', () => {
  const results = ['git status', 'git diff --stat', 'git status', 'git commit -m "x"', 'npm test'].map((detail) => permission('bypass', detail))
  assert.equal(autoPermissionSentence('Axiom', results), 'Axiom rodou 5 comandos (git status, git diff, git commit, …)')
  assert.equal(autoPermissionSentence('Axiom', [permission('bypass', 'npm test')]), 'Axiom rodou 1 comando (npm test)')
})
