// Falhas cobertas: resumo com markdown cru ou com o título no lugar do texto, identificador com "_" mutilado,
// plano de card concluído com selo "pronto", proposta com selo de coluna e aba de tipo no singular.
// Execução: cd desktop && node --test tests/artifact-card-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { plainSummary, kindTabLabel } = jiti('../src/features/artifacts/artifactsCopy.ts')
const { planChip } = jiti('../src/features/artifacts/planProgress.ts')

test('resumo pula o título e tira a marcação', () => {
  assert.equal(plainSummary('## Causa raiz\nEm `emit.rs` o **AgentState** não leva o item.'), 'Em emit.rs o AgentState não leva o item.')
  assert.equal(plainSummary('# Pesquisa: Stream-JSON vs PTY ## Contexto atual - O Vokia usa stdin'), 'Pesquisa: Stream-JSON vs PTY Contexto atual - O Vokia usa stdin')
})

test('resumo mantém identificadores e listas legíveis', () => {
  assert.equal(plainSummary('- usa `run_turns` com a chave\n- e o `base_command`'), 'usa run_turns com a chave e o base_command')
  assert.equal(plainSummary('Texto simples, sem marcação.'), 'Texto simples, sem marcação.')
  assert.equal(plainSummary(''), '')
})

test('plano aceito usa a coluna do card; proposta continua proposta', () => {
  assert.deepEqual(planChip('vinculado', 'Done'), { label: 'concluído', status: 'trabalhando' })
  assert.deepEqual(planChip('vinculado', 'In progress'), { label: 'em andamento', status: 'lote' })
  assert.equal(planChip('vinculado', 'Backlog').label, 'pronto')
  assert.equal(planChip('proposto', 'Done').label, 'proposta')
})

test('abas de tipo no plural, com o rótulo do backend para tipo novo', () => {
  assert.equal(kindTabLabel('documento', 'Documento'), 'Documentos')
  assert.equal(kindTabLabel('decisao', 'Decisão'), 'Decisões')
  assert.equal(kindTabLabel('especificacao', 'Especificação'), 'Especificações')
  assert.equal(kindTabLabel('roteiro', 'Roteiro'), 'Roteiro')
})
