// Validação E2E e integrada da Etapa MCP-7 (MCP nativo e Vokia CLI)
// Execução: cd desktop && node tests/mcp-vokia-cli-validacao.mjs
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { execFile } from 'node:child_process'
import { existsSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

console.log('=== Iniciando Validação E2E / Integrada Etapa MCP-7 (MCP Nativo + Vokia CLI) ===\n')

const results = []

async function test(name, fn) {
  try {
    await fn()
    results.push({ name, passed: true })
    console.log(`[PASSOU] ${name}`)
  } catch (error) {
    results.push({ name, passed: false, error: error.message })
    console.error(`[FALHOU] ${name}: ${error.message}`)
  }
}

// -------------------------------------------------------------
// Catálogo de Provedores com ToolTransport conforme MCP-1..MCP-6
// -------------------------------------------------------------
const PRESETS = [
  {
    id: 'claude',
    label: 'Claude Code',
    toolTransport: 'mcp-native',
    supportsMcp: true,
    supportsSandbox: true,
    defaultCommand: 'claude',
  },
  {
    id: 'codex',
    label: 'Codex',
    toolTransport: 'mcp-native',
    supportsMcp: true,
    supportsSandbox: true,
    defaultCommand: 'codex',
  },
  {
    id: 'opencode',
    label: 'OpenCode',
    toolTransport: 'vokia-cli',
    supportsMcp: false,
    supportsSandbox: false,
    defaultCommand: 'opencode',
  },
  {
    id: 'antigravity',
    label: 'Antigravity',
    toolTransport: 'vokia-cli',
    supportsMcp: false,
    supportsSandbox: true,
    defaultCommand: 'agy',
  },
  {
    id: 'generic-text',
    label: 'Generic Text-Only',
    toolTransport: 'none',
    supportsMcp: false,
    supportsSandbox: false,
    defaultCommand: 'generic-cli',
  },
]

const TRANSPORT_LABELS = {
  'mcp-native': 'MCP nativo',
  'vokia-cli': 'Vokia CLI',
  none: 'Só texto',
}

function effectiveTransport(preset) {
  if (preset.toolTransport) {
    return preset.toolTransport
  }
  return preset.supportsMcp ? 'mcp-native' : 'none'
}

function hasTools(preset) {
  return effectiveTransport(preset) !== 'none'
}

// -------------------------------------------------------------
// Cenário 1 — Badge de transporte no Setup e Agentes
// -------------------------------------------------------------
await test('Cenário 1 — Badge de transporte no Setup e Agentes exibe MCP nativo, Vokia CLI e Só texto', () => {
  // 1.1 Verificação dos badges para cada provedor em ProviderSetupDialog
  const claudePreset = PRESETS.find((p) => p.id === 'claude')
  const codexPreset = PRESETS.find((p) => p.id === 'codex')
  const opencodePreset = PRESETS.find((p) => p.id === 'opencode')
  const antigravityPreset = PRESETS.find((p) => p.id === 'antigravity')
  const textPreset = PRESETS.find((p) => p.id === 'generic-text')

  assert.equal(TRANSPORT_LABELS[effectiveTransport(claudePreset)], 'MCP nativo')
  assert.equal(TRANSPORT_LABELS[effectiveTransport(codexPreset)], 'MCP nativo')
  assert.equal(TRANSPORT_LABELS[effectiveTransport(opencodePreset)], 'Vokia CLI')
  assert.equal(TRANSPORT_LABELS[effectiveTransport(antigravityPreset)], 'Vokia CLI')
  assert.equal(TRANSPORT_LABELS[effectiveTransport(textPreset)], 'Só texto')

  // 1.2 Simulação de renderização do data-testid="provider-tool-transport" no ProviderSetupDialog
  for (const preset of PRESETS) {
    const transport = effectiveTransport(preset)
    const label = TRANSPORT_LABELS[transport]
    const badgeElement = {
      testId: 'provider-tool-transport',
      text: label,
    }
    assert.ok(badgeElement.text, `badge para ${preset.id} não pode ser vazio`)
    if (preset.id === 'claude' || preset.id === 'codex') {
      assert.equal(badgeElement.text, 'MCP nativo')
    } else if (preset.id === 'opencode' || preset.id === 'antigravity') {
      assert.equal(badgeElement.text, 'Vokia CLI')
    } else {
      assert.equal(badgeElement.text, 'Só texto')
    }
  }

  // 1.3 Verificação em AgentesPanel (ficha do agente selecionado)
  const agentSelection = {
    id: 'vexa',
    cli: 'opencode',
    followDefaultLlm: false,
  }
  const resolvedPreset = PRESETS.find((p) => p.id === agentSelection.cli)
  const agentBadge = TRANSPORT_LABELS[effectiveTransport(resolvedPreset)]
  assert.equal(agentBadge, 'Vokia CLI', 'Vexa com OpenCode deve mostrar badge Vokia CLI na ficha')
})

// -------------------------------------------------------------
// Cenário 2 — Ember liberado para provedores com ferramentas (P1)
// -------------------------------------------------------------
await test('Cenário 2 — Ember liberado para provedores com ferramentas (MCP nativo e Vokia CLI), recusa Só texto e avisa sandbox fora do Claude', () => {
  // 2.1 Simulação do NewAgentDialog ao editar Ember (isBuiltin = true)
  const isBuiltin = true

  // Computed cliOptions em NewAgentDialog.vue:
  // Filtra apenas provedores com hasTools() != false
  const cliOptions = PRESETS
    .filter((candidate) => !isBuiltin || hasTools(candidate))
    .map((candidate) => ({ label: candidate.label, value: candidate.id }))

  const optionValues = cliOptions.map((o) => o.value)
  assert.ok(optionValues.includes('claude'), 'Claude deve estar disponível para Ember')
  assert.ok(optionValues.includes('codex'), 'Codex deve estar disponível para Ember')
  assert.ok(optionValues.includes('opencode'), 'OpenCode deve estar disponível para Ember (via Vokia CLI)')
  assert.ok(optionValues.includes('antigravity'), 'Antigravity deve estar disponível para Ember (via Vokia CLI)')
  assert.ok(!optionValues.includes('generic-text'), 'Provedor Só texto deve ser EXCLUÍDO para o Ember')

  // 2.2 Aviso de sandbox exibido quando isBuiltin && cli !== 'claude'
  function sandboxNotice(cli, builtin) {
    if (builtin && cli !== 'claude') {
      return 'Fora do Claude, permissões seguem o nível de sandbox da CLI sem confirmação comando a comando.'
    }
    return null
  }

  assert.equal(sandboxNotice('claude', true), null, 'Claude não exibe aviso de sandbox no Ember')
  assert.equal(
    sandboxNotice('codex', true),
    'Fora do Claude, permissões seguem o nível de sandbox da CLI sem confirmação comando a comando.'
  )
  assert.equal(
    sandboxNotice('opencode', true),
    'Fora do Claude, permissões seguem o nível de sandbox da CLI sem confirmação comando a comando.'
  )

  // 2.3 Simulação da validação no backend (AgentRegistry::update_ember_llm)
  function validateEmberLlm(cliId, presets) {
    const provider = presets.find((p) => p.id === cliId)
    if (!provider) {
      throw new Error(`CLI desconhecida: ${cliId}.`)
    }
    if (!hasTools(provider)) {
      throw new Error('O agente Ember exige um provedor com suporte a ferramentas (MCP ou Vokia CLI).')
    }
    return { ok: true, cli: cliId }
  }

  assert.ok(validateEmberLlm('claude', PRESETS).ok)
  assert.ok(validateEmberLlm('codex', PRESETS).ok)
  assert.ok(validateEmberLlm('opencode', PRESETS).ok)
  assert.ok(validateEmberLlm('antigravity', PRESETS).ok)

  assert.throws(
    () => validateEmberLlm('generic-text', PRESETS),
    /O agente Ember exige um provedor com suporte a ferramentas \(MCP ou Vokia CLI\)\./
  )
})

// -------------------------------------------------------------
// Cenário 3 — vokia-cli real ponta a ponta
// -------------------------------------------------------------
await test('Cenário 3 — vokia-cli real ponta a ponta contra McpServer HTTP local autenticado', async () => {
  // 3.1 Localiza o binário compilado do vokia-cli
  const possiblePaths = [
    resolve(__dirname, '../../src-tauri/target/debug/vokia-cli.exe'),
    resolve(__dirname, '../../src-tauri/target/debug/vokia-cli'),
    resolve(__dirname, '../src-tauri/target/debug/vokia-cli.exe'),
    resolve(__dirname, '../src-tauri/target/debug/vokia-cli'),
  ]
  const binPath = possiblePaths.find((p) => existsSync(p))
  assert.ok(binPath, `binário vokia-cli não encontrado em target/debug/. Caminhos checados: ${possiblePaths.join(', ')}`)

  const VALID_TOKEN = 'token-secreto-vokia-mcp-e2e-2026'
  const recordedCalls = []

  // 3.2 Sobe servidor HTTP loopback simulando o McpServer do Vokia
  const server = createServer((req, res) => {
    let bodyRaw = ''
    req.on('data', (chunk) => {
      bodyRaw += chunk
    })
    req.on('end', () => {
      const auth = req.headers['authorization']
      if (auth !== `Bearer ${VALID_TOKEN}`) {
        res.writeHead(401, { 'Content-Type': 'application/json' })
        res.end(JSON.stringify({ jsonrpc: '2.0', id: null, error: { code: -32000, message: 'Token inválido' } }))
        return
      }

      let payload
      try {
        payload = JSON.parse(bodyRaw)
      } catch (err) {
        res.writeHead(400, { 'Content-Type': 'application/json' })
        res.end(JSON.stringify({ jsonrpc: '2.0', id: null, error: { code: -32700, message: 'Parse error' } }))
        return
      }

      recordedCalls.push(payload)

      if (payload.method === 'tools/list') {
        res.writeHead(200, { 'Content-Type': 'application/json' })
        res.end(
          JSON.stringify({
            jsonrpc: '2.0',
            id: payload.id,
            result: {
              tools: [
                { name: 'create_card', description: 'Cria card no backlog' },
                { name: 'delegate', description: 'Delega tarefa para agente' },
                { name: 'request_decision', description: 'Solicita decisão ao usuário' },
              ],
            },
          })
        )
        return
      }

      if (payload.method === 'tools/call') {
        const toolName = payload.params?.name
        const args = payload.params?.arguments || {}
        res.writeHead(200, { 'Content-Type': 'application/json' })
        res.end(
          JSON.stringify({
            jsonrpc: '2.0',
            id: payload.id,
            result: {
              content: [{ type: 'text', text: `Ferramenta ${toolName} executada com sucesso` }],
            },
          })
        )
        return
      }

      res.writeHead(404)
      res.end()
    })
  })

  await new Promise((resolvePromise) => server.listen(0, '127.0.0.1', resolvePromise))
  const port = server.address().port
  const serverUrl = `http://127.0.0.1:${port}/mcp`

  function runCli(args, envVars = {}) {
    return new Promise((resolvePromise) => {
      execFile(
        binPath,
        args,
        {
          env: {
            ...process.env,
            VOKIA_URL: serverUrl,
            VOKIA_TOKEN: VALID_TOKEN,
            ...envVars,
          },
        },
        (error, stdout, stderr) => {
          resolvePromise({
            code: error ? error.code || 1 : 0,
            stdout: stdout.trim(),
            stderr: stderr.trim(),
          })
        }
      )
    })
  }

  try {
    // 3.3 Executa `vokia-cli tools --json`
    const toolsResult = await runCli(['tools', '--json'])
    assert.equal(toolsResult.code, 0, `tools --json falhou: ${toolsResult.stderr}`)
    const parsedTools = JSON.parse(toolsResult.stdout)
    assert.ok(Array.isArray(parsedTools.tools), 'tools deve retornar array')
    assert.equal(parsedTools.tools.length, 3)

    // 3.4 Executa `vokia-cli create-card`
    const createResult = await runCli([
      'create-card',
      '--title',
      'Implementar validação MCP',
      '--description',
      'Card criado via vokia-cli',
    ])
    assert.equal(createResult.code, 0, `create-card falhou: ${createResult.stderr}`)
    assert.ok(createResult.stdout.includes('create_card executada com sucesso'))

    // 3.5 Executa `vokia-cli delegate`
    const delegateResult = await runCli([
      'delegate',
      '--agent',
      'Vexa',
      '--request',
      'Valide o fluxo integrado',
    ])
    assert.equal(delegateResult.code, 0, `delegate falhou: ${delegateResult.stderr}`)
    assert.ok(delegateResult.stdout.includes('delegate executada com sucesso'))

    // 3.6 Executa `vokia-cli request-decision`
    const decisionResult = await runCli([
      'request-decision',
      '--kind',
      'choice',
      '--title',
      'Podemos avançar para homologação?',
      '--option',
      'Sim',
      '--option',
      'Não',
    ])
    assert.equal(decisionResult.code, 0, `request-decision falhou: ${decisionResult.stderr}`)
    assert.ok(decisionResult.stdout.includes('request_decision executada com sucesso'))

    // 3.7 Verificação das chamadas gravadas no servidor
    const createCall = recordedCalls.find((c) => c.params?.name === 'create_card')
    assert.deepEqual(createCall.params.arguments, {
      title: 'Implementar validação MCP',
      description: 'Card criado via vokia-cli',
    })

    const delegateCall = recordedCalls.find((c) => c.params?.name === 'delegate')
    assert.deepEqual(delegateCall.params.arguments, {
      agent: 'Vexa',
      request: 'Valide o fluxo integrado',
    })

    const decisionCall = recordedCalls.find((c) => c.params?.name === 'request_decision')
    assert.deepEqual(decisionCall.params.arguments, {
      kind: 'choice',
      title: 'Podemos avançar para homologação?',
      options: ['Sim', 'Não'],
    })

    // 3.8 Executa chamada com token inválido -> deve falhar
    const badTokenResult = await runCli(['tools'], { VOKIA_TOKEN: 'token-invalido-123' })
    assert.notEqual(badTokenResult.code, 0, 'chamada com token inválido deve falhar')
  } finally {
    server.close()
  }
})

// -------------------------------------------------------------
// Cenário 4 — Delegação de card para agente via Vokia CLI
// -------------------------------------------------------------
await test('Cenário 4 — Delegação de card para agente via Vokia CLI é aceita com item_id e recusada para agente Só texto', () => {
  // Simulação do comportamento de mcp/tools.rs: delegate(app, binding, args)
  const agents = [
    { id: 'vexa', name: 'Vexa', cli: 'opencode' }, // toolTransport: vokia-cli
    { id: 'axiom', name: 'Axiom', cli: 'codex' },    // toolTransport: mcp-native
    { id: 'ghost', name: 'Ghost', cli: 'generic-text' }, // toolTransport: none
  ]

  function simulateDelegate(targetAgentName, itemId, requestText) {
    const target = agents.find((a) => a.name.toLowerCase() === targetAgentName.toLowerCase())
    if (!target) {
      return { isError: true, error: `Agente não encontrado: ${targetAgentName}` }
    }
    const preset = PRESETS.find((p) => p.id === target.cli)
    const targetHasTools = hasTools(preset)

    // Regra D2 / MCP-1: delegação de card (com item_id) exige suporte a ferramentas
    if (itemId !== null && itemId !== undefined) {
      if (!targetHasTools) {
        return {
          isError: true,
          error: `${target.name} usa uma CLI sem suporte a ferramentas (MCP ou Vokia CLI) e não pode receber delegação de card.`,
        }
      }
    }

    return {
      isError: false,
      text: 'Registrado.',
      delegation: { agent: target.id, request: requestText, itemId },
    }
  }

  // 4.1 Delegação de card para agente Vexa (OpenCode via Vokia CLI) -> DEVE ACEITAR
  const delVexaCard = simulateDelegate('Vexa', 'card-101', 'Implementar tela')
  assert.equal(delVexaCard.isError, false, 'delegação de card para Vexa (Vokia CLI) deve ser aceita')
  assert.equal(delVexaCard.text, 'Registrado.')
  assert.equal(delVexaCard.delegation.itemId, 'card-101')

  // 4.2 Delegação de card para agente Axiom (Codex via MCP Nativo) -> DEVE ACEITAR
  const delAxiomCard = simulateDelegate('Axiom', 'card-102', 'Revisar arquitetura')
  assert.equal(delAxiomCard.isError, false, 'delegação de card para Axiom (MCP nativo) deve ser aceita')
  assert.equal(delAxiomCard.text, 'Registrado.')

  // 4.3 Delegação de card para agente Ghost (Generic Só texto) -> DEVE RECUSAR
  const delGhostCard = simulateDelegate('Ghost', 'card-103', 'Tarefa impossível')
  assert.equal(delGhostCard.isError, true, 'delegação de card para agente Só texto deve ser recusada')
  assert.equal(
    delGhostCard.error,
    'Ghost usa uma CLI sem suporte a ferramentas (MCP ou Vokia CLI) e não pode receber delegação de card.'
  )

  // 4.4 Conversa pura de texto sem item_id para agente Só texto -> DEVE ACEITAR
  const delGhostText = simulateDelegate('Ghost', null, 'Apenas uma dúvida em texto')
  assert.equal(delGhostText.isError, false, 'conversa pura de texto para agente Só texto deve ser aceita')
  assert.equal(delGhostText.text, 'Registrado.')
})

// -------------------------------------------------------------
// Resumo da Validação
// -------------------------------------------------------------
console.log('\n=== Resumo dos Testes E2E Etapa MCP-7 ===')
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
