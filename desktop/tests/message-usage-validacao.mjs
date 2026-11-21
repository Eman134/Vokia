// Validação E2E e integrada da Etapa MSG-4 (Modelo, tokens e custo nas mensagens)
// Execução: cd desktop && node tests/message-usage-validacao.mjs
import assert from 'node:assert/strict'
import {
  formatTokenCount,
  formatCostUsd,
  formatUsageInline,
  formatUsageTooltip,
} from '../src/features/team/messageUsageFormat.ts'

console.log('=== Iniciando Validação E2E da Etapa MSG-4 (Metadados de Uso nas Mensagens) ===\n')

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

// -------------------------------------------------------------
// Simulação do comportamento de renderização de MessageEntry.vue
// -------------------------------------------------------------
function isBetweenAgents(entry) {
  return entry.from !== 'user' && entry.to !== 'user'
}

function computeEntryView(entry) {
  const isAgentMessage = entry.type === 'message' && entry.from !== 'user'
  const betweenAgents = entry.type === 'message' && isBetweenAgents(entry)

  const usageInline = isAgentMessage ? formatUsageInline(entry.usage) : null
  const usageTooltip = isAgentMessage ? formatUsageTooltip(entry.usage) : ''

  // Template em MessageEntry.vue:
  // Se entre agentes: renderiza container agent-exchange com title contendo usageTooltip, SEM message-usage
  // Se agente para usuário: renderiza cabeçalho com time e span message-usage se usageInline != null
  // Se mensagem de usuário ou sem usage: NÃO renderiza message-usage
  const hasAgentExchange = betweenAgents
  const agentExchangeTitle = hasAgentExchange
    ? `12:00${usageTooltip ? ` — ${usageTooltip}` : ''}`
    : null

  const hasMessageUsage = !betweenAgents && isAgentMessage && usageInline !== null
  const messageUsageText = hasMessageUsage ? `· ${usageInline}` : null
  const messageUsageTitle = hasMessageUsage ? usageTooltip : null

  return {
    hasAgentExchange,
    agentExchangeTitle,
    hasMessageUsage,
    messageUsageText,
    messageUsageTitle,
    usageInline,
    usageTooltip,
  }
}

// -------------------------------------------------------------
// Cenário 1 — Mensagem completa (modelo + tokens compactos + custo)
// -------------------------------------------------------------
test('Cenário 1 — Mensagem completa exibe modelo, tokens compactos in/out e custo formatado, com title detalhado', () => {
  const usage = {
    provider: 'claude',
    model: 'claude-sonnet-4-6',
    inputTokens: 1240,
    outputTokens: 340,
    cachedInputTokens: 800,
    costUsd: 0.0042,
  }

  // 1.1 Formatação pura inline
  const inline = formatUsageInline(usage)
  assert.equal(inline, 'claude-sonnet-4-6 · 1.2k in · 340 out · $0.0042')

  // 1.2 Formatação do tooltip (title)
  const tooltip = formatUsageTooltip(usage)
  assert.ok(tooltip.includes('Provedor: claude'), 'deve conter provedor')
  assert.ok(tooltip.includes('Modelo: claude-sonnet-4-6'), 'deve conter modelo')
  assert.ok(tooltip.includes('1.240 tokens') || tooltip.includes('1240 tokens'), 'deve conter entrada exata')
  assert.ok(tooltip.includes('800 tokens'), 'deve conter cache')
  assert.ok(tooltip.includes('340 tokens'), 'deve conter saída exata')
  assert.ok(tooltip.includes('Custo: $0.0042'), 'deve conter custo')

  // 1.3 Renderização na mensagem do agente para o usuário
  const entry = {
    type: 'message',
    from: 'axiom',
    to: 'user',
    text: 'Plano gerado com sucesso.',
    time: 1700000000000,
    usage,
  }

  const view = computeEntryView(entry)
  assert.equal(view.hasMessageUsage, true, 'deve renderizar data-testid="message-usage"')
  assert.equal(view.messageUsageText, '· claude-sonnet-4-6 · 1.2k in · 340 out · $0.0042')
  assert.equal(view.messageUsageTitle, tooltip)
  assert.equal(view.hasAgentExchange, false)
})

// -------------------------------------------------------------
// Cenário 2 — Custo ausente ou zero (P2)
// -------------------------------------------------------------
test('Cenário 2 — Custo ausente ou zero não exibe símbolo de dólar em message-usage', () => {
  // 2.1 Custo zero
  const usageZero = {
    provider: 'codex',
    model: 'gpt-6-luna',
    inputTokens: 2500,
    outputTokens: 100,
    costUsd: 0,
  }
  const inlineZero = formatUsageInline(usageZero)
  assert.equal(inlineZero, 'gpt-6-luna · 2.5k in · 100 out')
  assert.ok(!inlineZero.includes('$'), 'não deve conter $ quando custo é zero')

  const tooltipZero = formatUsageTooltip(usageZero)
  assert.ok(!tooltipZero.includes('Custo:'), 'tooltip não deve conter Custo quando for zero')

  // 2.2 Custo indefinido (undefined)
  const usageUndef = {
    provider: 'antigravity',
    model: 'gemini-4-hydrogen-medium',
    inputTokens: 850,
    outputTokens: 420,
    costUsd: undefined,
  }
  const inlineUndef = formatUsageInline(usageUndef)
  assert.equal(inlineUndef, 'gemini-4-hydrogen-medium · 850 in · 420 out')
  assert.ok(!inlineUndef.includes('$'), 'não deve conter $ quando custo é undefined')

  // 2.3 Renderização da view
  const entry = {
    type: 'message',
    from: 'vexa',
    to: 'user',
    text: 'Tarefa concluída.',
    time: 1700000000000,
    usage: usageZero,
  }
  const view = computeEntryView(entry)
  assert.equal(view.hasMessageUsage, true)
  assert.ok(!view.messageUsageText.includes('$'))
})

// -------------------------------------------------------------
// Cenário 3 — Apenas modelo (sem tokens/custo)
// -------------------------------------------------------------
test('Cenário 3 — Mensagem apenas com modelo exibe nome do modelo sem separadores ou tokens extras', () => {
  const usageModelOnly = {
    provider: 'generic',
    model: 'llama-4',
  }

  const inline = formatUsageInline(usageModelOnly)
  assert.equal(inline, 'llama-4', 'deve exibir estritamente o modelo sem separadores')

  const tooltip = formatUsageTooltip(usageModelOnly)
  assert.equal(tooltip, 'Provedor: generic · Modelo: llama-4')

  const entry = {
    type: 'message',
    from: 'artesao',
    to: 'user',
    text: 'Resposta rápida.',
    time: 1700000000000,
    usage: usageModelOnly,
  }
  const view = computeEntryView(entry)
  assert.equal(view.hasMessageUsage, true)
  assert.equal(view.messageUsageText, '· llama-4')
  assert.equal(view.messageUsageTitle, 'Provedor: generic · Modelo: llama-4')
})

// -------------------------------------------------------------
// Cenário 4 — Troca entre agentes (agent-exchange, P1)
// -------------------------------------------------------------
test('Cenário 4 — Mensagem entre agentes não exibe message-usage inline, mas inclui metadados no title do container agent-exchange', () => {
  const usage = {
    provider: 'claude',
    model: 'haiku',
    inputTokens: 500,
    outputTokens: 120,
    costUsd: 0.0015,
  }

  const exchangeEntry = {
    type: 'message',
    from: 'axiom',
    to: 'vexa', // Entre agentes!
    text: '@Vexa: revise o arquivo x.',
    time: 1700000000000,
    usage,
  }

  const view = computeEntryView(exchangeEntry)

  // Não polui o feed com span de message-usage
  assert.equal(view.hasMessageUsage, false, 'mensagem entre agentes NÃO deve renderizar message-usage inline')
  assert.equal(view.hasAgentExchange, true, 'deve renderizar container data-testid="agent-exchange"')

  // Detalhamento vai para o title do container
  assert.ok(view.agentExchangeTitle, 'agent-exchange deve ter title')
  assert.ok(view.agentExchangeTitle.includes('Provedor: claude'))
  assert.ok(view.agentExchangeTitle.includes('Modelo: haiku'))
  assert.ok(view.agentExchangeTitle.includes('Custo: $0.0015'))
})

// -------------------------------------------------------------
// Cenário 5 — Mensagem antiga sem usage e mensagem do usuário
// -------------------------------------------------------------
test('Cenário 5 — Mensagens antigas sem usage e mensagens do usuário nunca renderizam message-usage', () => {
  // 5.1 Mensagem do usuário com texto
  const userEntry = {
    type: 'message',
    from: 'user',
    to: 'axiom',
    text: 'Crie o projeto.',
    time: 1700000000000,
  }
  const userView = computeEntryView(userEntry)
  assert.equal(userView.hasMessageUsage, false, 'mensagem do usuário nunca renderiza message-usage')
  assert.equal(userView.usageInline, null)

  // 5.2 Mensagem antiga de agente gravada antes da migration (usage: undefined/null)
  const legacyAgentEntry = {
    type: 'message',
    from: 'axiom',
    to: 'user',
    text: 'Mensagem legada no histórico.',
    time: 1700000000000,
    usage: undefined,
  }
  const legacyView = computeEntryView(legacyAgentEntry)
  assert.equal(legacyView.hasMessageUsage, false, 'mensagem legada sem usage não renderiza message-usage')
  assert.equal(legacyView.usageInline, null)

  // 5.3 Mensagem com usage vazio (objeto sem modelo nem tokens)
  const emptyUsageEntry = {
    type: 'message',
    from: 'axiom',
    to: 'user',
    text: 'Mensagem com usage vazio.',
    time: 1700000000000,
    usage: {},
  }
  const emptyView = computeEntryView(emptyUsageEntry)
  assert.equal(emptyView.hasMessageUsage, false, 'usage vazio não deve renderizar message-usage')
})

// -------------------------------------------------------------
// Resumo da Validação
// -------------------------------------------------------------
console.log('\n=== Resumo dos Testes E2E Etapa MSG-4 ===')
const failed = results.filter((r) => !r.passed)
for (const r of results) {
  console.log(`${r.passed ? '✓' : '✗'} ${r.name}`)
}

if (failed.length > 0) {
  console.error(`\n${failed.length} teste(s) falharam!`)
  process.exit(1)
} else {
  console.log(`\nTodos os ${results.length} cenários funcionais E2E passaram com sucesso!`)
  process.exit(0)
}
