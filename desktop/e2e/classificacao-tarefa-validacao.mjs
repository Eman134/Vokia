// Validação integrada das regras CLS-1 a CLS-3 e dos contratos CLS-2 de persistência.
// Execução: cd desktop && node e2e/classificacao-tarefa-validacao.mjs
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createJiti } from 'jiti'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const desktopRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const jiti = createJiti(import.meta.url)
const { entryInvolves, screenEntries } = jiti('../src/features/team/timeline.ts')
const { GENERAL_CHANNEL_NAME } = jiti('../src/features/channels/channelRouting.ts')
const channels = [{ id: 2, name: 'desenvolvimento', members: ['ember', 'vexa'] }]

function taskResult(channelId) {
  return {
    type: 'task-result',
    projectId: 1,
    objective: 'Corrigir o filtro',
    concluded: true,
    turns: 2,
    agents: ['ember', 'vexa'],
    time: 100,
    channelId,
  }
}

function isVisible(entry, screen) {
  return screenEntries([entry], screen, channels).includes(entry)
}

function validateTimeline() {
  const generalResult = taskResult(null)
  assert.equal(entryInvolves(generalResult, 'ember'), true)
  assert.equal(entryInvolves(generalResult, 'vexa'), true)
  assert.equal(entryInvolves(generalResult, 'kairo'), false)
  assert.equal(isVisible(generalResult, { kind: 'all' }), true)
  assert.equal(isVisible(generalResult, { kind: 'channel', channel: GENERAL_CHANNEL_NAME }), true)
  assert.equal(isVisible(generalResult, { kind: 'channel', channel: 2 }), false)
  for (const agent of generalResult.agents) {
    assert.equal(isVisible(generalResult, { kind: 'agent', agent }), true, `${agent} deve ver o resultado`)
  }
  assert.equal(isVisible(generalResult, { kind: 'agent', agent: 'kairo' }), false)

  const channelResult = taskResult(2)
  assert.equal(entryInvolves(channelResult, 'ember'), true)
  assert.equal(entryInvolves(channelResult, 'vexa'), true)
  assert.equal(entryInvolves(channelResult, 'kairo'), false)
  assert.equal(isVisible(channelResult, { kind: 'channel', channel: 2 }), true)
  assert.equal(isVisible(channelResult, { kind: 'channel', channel: GENERAL_CHANNEL_NAME }), false)
  for (const agent of channelResult.agents) {
    assert.equal(isVisible(channelResult, { kind: 'agent', agent }), true, `${agent} deve ver o resultado`)
  }
  assert.equal(isVisible(channelResult, { kind: 'agent', agent: 'kairo' }), false)
  console.log('[PASSOU] TaskResult aparece no canal e nas conversas dos participantes corretos.')
}

function runRustContract(filter, description) {
  const output = execFileSync(
    'rtk',
    ['cargo', 'test', '--quiet', '--manifest-path', 'src-tauri/Cargo.toml', filter],
    { cwd: desktopRoot, encoding: 'utf8', stdio: 'pipe' },
  )
  console.log(`[PASSOU] ${description}`)
  if (output.trim()) console.log(output.trim())
}

validateTimeline()
runRustContract('cls1_classifica_saudacao_pergunta_e_pedidos_de_trabalho', 'classificação de Conversa e Demanda')
runRustContract('task_result', 'Settlement::Ended e persistência/reconstrução de TaskResult')
console.log('CLS-4 integrado passou.')
