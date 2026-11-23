// Validação do card item-52: toda decisão com opções aceita resposta digitada
// Execução: cd desktop && node tests/decision-typed-answer-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { acceptsTypedAnswer, chosenAnswer, optionMessage } = jiti('../src/features/decisions/decisionCopy.ts')

const cases = [
  ['escolha, pergunta e revisão com opções aceitam texto', () => {
    assert.equal(acceptsTypedAnswer('escolha', 3), true)
    assert.equal(acceptsTypedAnswer('pergunta', 0), true)
    assert.equal(acceptsTypedAnswer('revisao', 2), true)
  }],
  ['revisão sem opções, aprovação, validação e permissão ficam como estão', () => {
    for (const kind of ['aprovacao', 'validacao', 'permissao']) assert.equal(acceptsTypedAnswer(kind, 3), false)
    assert.equal(acceptsTypedAnswer('revisao', 0), false)
  }],
  ['sem opção nem texto a resposta é vazia (botão desativado)', () => {
    assert.equal(chosenAnswer(null, ''), '')
    assert.equal(chosenAnswer(null, '   '), '')
  }],
  ['opção marcada vale quando não há texto', () => assert.equal(chosenAnswer('A', ''), 'A')],
  ['texto digitado vale, sem espaços nas pontas', () => assert.equal(chosenAnswer(null, '  outra  '), 'outra')],
  ['texto tem prioridade sobre a opção marcada', () => assert.equal(chosenAnswer('A', 'outra'), 'outra')],
  ['mensagem ao usuário segue o tipo', () => {
    assert.equal(optionMessage('escolha', 'Ember', 'x'), 'Ember segue com: x')
    assert.equal(optionMessage('pergunta', 'Ember', 'x'), 'Ember vai usar: x')
  }],
]
let failed = 0
for (const [name, fn] of cases) {
  try { fn(); console.log(`[PASSOU] ${name}`) } catch (e) { failed++; console.error(`[FALHOU] ${name}: ${e.message}`) }
}
process.exit(failed ? 1 : 0)
