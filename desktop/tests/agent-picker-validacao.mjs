// Validação do requisito 086: lógica pura do AgentPicker (filtro, ordenação ocupados primeiro, chip LLM, Todos os agentes)
// Execução: cd desktop && node tests/agent-picker-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)

const {
  ALL_AGENTS_OPTION_ID,
  DEFAULT_ALL_AGENTS_LABEL,
  normalizeSearchText,
  presenceMetaOf,
  formatProviderLabel,
  formatShortModel,
  resolveAgentLlm,
  buildAgentPickerOptions,
  emptyAgentSearchMessage,
} = jiti('../src/features/agents/agentPicker.ts')

console.log('=== Iniciando Validação de agentPicker (086) ===\n')

const results = []

function test(name, fn) {
  try {
    fn()
    results.push({ name, passed: true })
    console.log(`[PASSOU] ${name}`)
  } catch (error) {
    results.push({ name, passed: false, error: error.message })
    console.error(`[FALHOU] ${name}: ${error.message}`)
  }
}

const PRESETS = [
  {
    id: 'claude',
    label: 'Claude',
    models: ['claude-sonnet-4-5-20250929', 'claude-opus-4-1'],
    defaultModel: 'claude-sonnet-4-5-20250929',
    defaultCommand: 'claude',
    tiers: { basic: 'claude-haiku-4-5', intermediate: 'claude-sonnet-4-5-20250929', advanced: 'claude-opus-4-1' },
    supportsMcp: true,
    supportsSandbox: true,
    builtinSeed: true,
  },
  {
    id: 'codex',
    label: 'Codex',
    models: ['gpt-5.3-codex'],
    defaultModel: 'gpt-5.3-codex',
    defaultCommand: 'codex',
    tiers: { basic: 'gpt-5-mini', intermediate: 'gpt-5.3-codex', advanced: 'gpt-5.3-codex' },
    supportsMcp: true,
    supportsSandbox: true,
    builtinSeed: true,
  },
]

function makeAgent(id, name, role, overrides = {}) {
  return {
    id,
    name,
    role,
    avatar: id,
    cli: 'claude',
    model: 'claude-sonnet-4-5-20250929',
    modelTier: null,
    command: 'claude',
    builtin: false,
    extraInstructions: '',
    followDefaultLlm: false,
    fallback: [],
    ...overrides,
  }
}

const ROSTER = [
  makeAgent('ember', 'Ember', 'Líder de Orquestração', { followDefaultLlm: true }),
  makeAgent('kairo', 'Kairo', 'Backend Rust', { cli: 'codex', model: 'gpt-5.3-codex' }),
  makeAgent('vexa', 'Vexa', 'Frontend Vue', { cli: 'claude', model: 'claude-sonnet-4-5-20250929' }),
  makeAgent('sentry', 'Sentry', 'Revisor de Código', { cli: 'claude', model: 'claude-opus-4-1' }),
  makeAgent('luma', 'Luma', 'Designer UX', { cli: 'gemini', model: 'gemini-2.5-pro' }),
  makeAgent('davi', 'Davi', 'Engenheiro QA', { cli: 'codex', model: 'gpt-5-mini' }),
]

test('normalizeSearchText remove acentos, espaços extras e normaliza caixa', () => {
  assert.equal(normalizeSearchText('  Líder de Orquestração  '), 'lider de orquestracao')
  assert.equal(normalizeSearchText('CÓDIGO'), 'codigo')
})

test('formatShortModel e formatProviderLabel produzem chips compactos', () => {
  assert.equal(formatProviderLabel('claude', PRESETS), 'Claude')
  assert.equal(formatProviderLabel('codex', PRESETS), 'Codex')
  assert.equal(formatProviderLabel('gemini', PRESETS), 'Gemini')
  assert.equal(formatShortModel('claude-sonnet-4-5-20250929'), 'Sonnet 4.5')
  assert.equal(formatShortModel('anthropic/claude-opus-4-1'), 'Opus 4.1')
  assert.equal(formatShortModel('gpt-5.3-codex'), 'GPT-5.3-codex')
})

test('resolveAgentLlm respeita override temporário, followDefaultLlm e configuração própria', () => {
  const activeLlm = { cli: 'claude', modelTier: 'advanced', model: null }

  const emberLlm = resolveAgentLlm(ROSTER[0], { presets: PRESETS, activeLlm })
  assert.equal(emberLlm.chipLabel, 'Claude · Opus 4.1')
  assert.equal(emberLlm.isOverride, false)

  const kairoLlm = resolveAgentLlm(ROSTER[1], { presets: PRESETS, activeLlm })
  assert.equal(kairoLlm.chipLabel, 'Codex · GPT-5.3-codex')

  const overridden = resolveAgentLlm(ROSTER[2], {
    presets: PRESETS,
    activeLlm,
    override: {
      agent: 'vexa',
      cli: 'codex',
      model: 'gpt-5.3-codex',
      originalCli: 'claude',
      originalModel: 'claude-sonnet-4-5-20250929',
      reason: 'usage-limit',
      expiresAt: Date.now() + 60_000,
    },
  })
  assert.equal(overridden.chipLabel, 'Codex · GPT-5.3-codex')
  assert.equal(overridden.isOverride, true)
})

test('presenceMetaOf diferencia trabalhando, aguardando, livre e pausado com prioridades claras', () => {
  const working = presenceMetaOf('thinking', false)
  const sending = presenceMetaOf('sending', false)
  const waiting = presenceMetaOf('waiting', false)
  const idle = presenceMetaOf('idle', false)
  const paused = presenceMetaOf('thinking', true)

  assert.equal(working.kind, 'working')
  assert.equal(sending.kind, 'working')
  assert.equal(waiting.kind, 'waiting')
  assert.equal(idle.kind, 'idle')
  assert.equal(paused.kind, 'paused')
  assert.ok(working.priority < waiting.priority)
  assert.ok(waiting.priority < idle.priority)
  assert.ok(idle.priority < paused.priority)
})

test('buildAgentPickerOptions filtra instantaneamente por nome, papel ou provedor em time com 6+ agentes', () => {
  const base = {
    agents: ROSTER,
    states: {},
    isPaused: () => false,
    presets: PRESETS,
  }

  const byProvider = buildAgentPickerOptions({ ...base, query: 'codex' })
  assert.deepEqual(
    byProvider.map((item) => item.id),
    ['kairo', 'davi'],
  )

  const byRole = buildAgentPickerOptions({ ...base, query: 'revisor' })
  assert.deepEqual(
    byRole.map((item) => item.id),
    ['sentry'],
  )

  const byAccentInsensitiveRole = buildAgentPickerOptions({ ...base, query: 'orquestracao' })
  assert.deepEqual(
    byAccentInsensitiveRole.map((item) => item.id),
    ['ember'],
  )

  const byName = buildAgentPickerOptions({ ...base, query: 'vex' })
  assert.deepEqual(
    byName.map((item) => item.id),
    ['vexa'],
  )
})

test('buildAgentPickerOptions ordena ocupados primeiro quando busyFirst está ativo', () => {
  const options = buildAgentPickerOptions({
    agents: ROSTER,
    query: '',
    states: {
      ember: 'idle',
      kairo: 'idle',
      vexa: 'waiting',
      sentry: 'thinking',
      luma: 'sending',
      davi: 'idle',
    },
    isPaused: (id) => id === 'kairo',
    presets: PRESETS,
    busyFirst: true,
    counts: { sentry: 2, davi: 1 },
  })

  assert.deepEqual(
    options.map((item) => item.id),
    ['sentry', 'luma', 'vexa', 'ember', 'davi', 'kairo'],
  )
  assert.equal(options[0].count, 2)
  assert.equal(options[4].count, 1)
  assert.equal(options[1].count, 0)
})

test('buildAgentPickerOptions suporta opção "Todos os agentes" e esconde quando busca não casa', () => {
  const withAll = buildAgentPickerOptions({
    agents: ROSTER,
    query: '',
    states: {},
    isPaused: () => false,
    presets: PRESETS,
    allowAll: true,
    counts: { kairo: 2, vexa: 3 },
  })

  assert.equal(withAll[0].kind, 'all')
  assert.equal(withAll[0].id, ALL_AGENTS_OPTION_ID)
  assert.equal(withAll[0].label, DEFAULT_ALL_AGENTS_LABEL)
  assert.equal(withAll[0].count, 5)
  assert.equal(withAll.length, ROSTER.length + 1)

  const filteredOut = buildAgentPickerOptions({
    agents: ROSTER,
    query: 'codex',
    states: {},
    isPaused: () => false,
    presets: PRESETS,
    allowAll: true,
  })
  assert.equal(filteredOut.some((item) => item.kind === 'all'), false)
  assert.deepEqual(
    filteredOut.map((item) => item.id),
    ['kairo', 'davi'],
  )
})

test('emptyAgentSearchMessage formata estado vazio conforme especificação', () => {
  assert.equal(emptyAgentSearchMessage('  xyz  '), 'Nenhum agente encontrado para «xyz»')
})

const failed = results.filter((r) => !r.passed)
console.log(`\n=== Resumo: ${results.length - failed.length}/${results.length} passaram ===`)
if (failed.length > 0) {
  process.exit(1)
}
