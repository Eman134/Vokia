// Falhas cobertas: abrir sempre a primeira etapa mesmo já feita, mais de uma etapa aberta ao mesmo tempo
// e selo "proposta"/"pronto" repetido em cada etapa quando é o mesmo estado do card.
// Execução: cd desktop && node --test tests/breakdown-accordion-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { firstPendingStepId, toggleStep, showsStepStatus } = jiti('../src/features/backlog/breakdownAccordion.ts')

const step = (id, status, flowState) => ({ id, status, flowState })

test('abre a primeira etapa ainda não concluída; tudo feito abre a primeira', () => {
  assert.equal(firstPendingStepId([step('1', 'concluído'), step('2', 'concluído'), step('3', 'pronto'), step('4', 'pronto')]), '3')
  assert.equal(firstPendingStepId([step('1', 'concluído'), step('2', 'concluído')]), '1')
  assert.equal(firstPendingStepId([]), undefined)
})

test('acordeão: abrir outra etapa fecha a anterior e clicar na aberta fecha', () => {
  assert.deepEqual([...toggleStep(new Set(['1']), '2')], ['2'])
  assert.deepEqual([...toggleStep(new Set(['2']), '2')], [])
  assert.deepEqual([...toggleStep(new Set(), '3')], ['3'])
})

test('o selo da etapa só aparece quando difere do estado do card', () => {
  assert.equal(showsStepStatus(step('1', 'proposta')), false)
  assert.equal(showsStepStatus(step('1', 'pronto')), false)
  assert.equal(showsStepStatus(step('1', 'em andamento')), true)
  assert.equal(showsStepStatus(step('1', 'concluído')), true)
  assert.equal(showsStepStatus(step('1', 'pronto', 'escalada')), true)
})
