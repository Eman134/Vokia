// Falhas cobertas: card com o título repetido na linha do plano e linha vazia quando só sobraria o separador.
// Execução: cd desktop && node --test tests/plan-line-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { linkedPlanLine } = jiti('../src/features/backlog/planLine.ts')

test('plano com o mesmo nome do card mostra só o estado', () => {
  assert.equal(linkedPlanLine('Corrigir login', { name: 'Corrigir login', meta: 'proposta aceita' }), 'proposta aceita')
  assert.equal(linkedPlanLine('Corrigir login', { name: ' corrigir LOGIN ', meta: 'proposta aceita' }), 'proposta aceita')
})

test('plano com outro nome mostra nome e estado', () => {
  assert.equal(linkedPlanLine('Corrigir login', { name: 'Plano de auth', meta: '3 etapas' }), 'Plano de auth · 3 etapas')
})

test('sem nada além do título não há linha', () => {
  assert.equal(linkedPlanLine('Corrigir login', { name: 'Corrigir login', meta: '' }), undefined)
  assert.equal(linkedPlanLine('Corrigir login', { name: 'Outro', meta: '' }), 'Outro')
})
