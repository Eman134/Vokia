// Falhas cobertas: busca que não acha "permissão" digitando "permissao", ID fora da busca, plano concluído no mesmo
// grupo de proposta, plano sem data derrubando a ordem, grupo vazio na tela e documento passando por filtro de estado.
// Execução: cd desktop && node --test tests/artifact-browse-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { matchesQuery, groupOf, matchesState, groupEntries } = jiti('../src/features/artifacts/artifactBrowse.ts')

const plan = (id, state, column, updatedAt, extra = {}) => ({ key: `plano:${id}`, id, title: `Plano ${id}`, summary: '', state, column, updatedAt, ...extra })
const doc = (id, updatedAt) => ({ key: `documento:${id}`, id, title: `Doc ${id}`, summary: '', updatedAt })

test('busca ignora acento e caixa, e olha título, id e resumo', () => {
  const entry = plan('item-49', 'vinculado', 'Ready', 1, { title: 'Pedidos de permissão', summary: '## Causa\nO `base_command` falha' })
  assert.ok(matchesQuery(entry, 'PERMISSAO'))
  assert.ok(matchesQuery(entry, 'item-49'))
  assert.ok(matchesQuery(entry, 'base_command'))
  assert.ok(!matchesQuery(entry, 'item-50'))
  assert.ok(matchesQuery(entry, '   '))
})

test('grupo vem do estado e da coluna do card', () => {
  assert.equal(groupOf(plan('a', 'proposto', 'Backlog', 1)), 'precisa')
  assert.equal(groupOf(plan('a', 'gerando', undefined, 1)), 'execucao')
  assert.equal(groupOf(plan('a', 'vinculado', 'In progress', 1)), 'execucao')
  assert.equal(groupOf(plan('a', 'vinculado', 'In review', 1)), 'execucao')
  assert.equal(groupOf(plan('a', 'vinculado', 'Ready', 1)), 'prontos')
  assert.equal(groupOf(plan('a', 'vinculado', 'Done', 1)), 'concluidos')
  assert.equal(groupOf(doc('d', 1)), 'documentos')
})

test('filtro de estado só deixa passar planos do grupo; documento só em "todos"', () => {
  assert.ok(matchesState(plan('a', 'proposto', 'Backlog', 1), 'proposta'))
  assert.ok(!matchesState(plan('a', 'vinculado', 'Done', 1), 'proposta'))
  assert.ok(matchesState(plan('a', 'vinculado', 'Done', 1), 'concluido'))
  assert.ok(matchesState(doc('d', 1), 'todos'))
  assert.ok(!matchesState(doc('d', 1), 'pronto'))
})

test('grupos saem na ordem, sem grupo vazio, mais recente primeiro e sem data por último', () => {
  const groups = groupEntries([
    plan('velho', 'vinculado', 'Ready', 10),
    plan('feito', 'vinculado', 'Done', 99),
    plan('novo', 'vinculado', 'Ready', 50),
    plan('semdata', 'vinculado', 'Ready', undefined),
    plan('pedido', 'proposto', 'Backlog', 5),
    doc('d', 7),
  ])
  assert.deepEqual(groups.map((group) => group.id), ['precisa', 'prontos', 'concluidos', 'documentos'])
  assert.deepEqual(groups[1].entries.map((entry) => entry.id), ['novo', 'velho', 'semdata'])
  assert.deepEqual(groupEntries([]), [])
})
