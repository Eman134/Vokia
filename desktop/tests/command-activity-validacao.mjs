// Validação da Etapa CMD-ACT-3 (Testes Rust e validação de frases/ícones de comando na atividade)
// Execução: cd desktop && node tests/command-activity-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)

const {
  commandPhrase,
  friendlyCommand,
} = jiti('../src/features/team/appCommandLabel.ts')

const {
  activityRows,
  activityClauses,
  activitySentence,
} = jiti('../src/features/team/activitySummary.ts')

console.log('=== Iniciando Validação da Etapa CMD-ACT-3 (Comandos na Atividade) ===\n')

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
// Testes de commandPhrase
// -------------------------------------------------------------
test('commandPhrase — Ação e entidade com ordem direta e invertida nos três estados', () => {
  // Ordem invertida: role_create
  assert.equal(commandPhrase('role_create', 'running'), 'Criando o papel')
  assert.equal(commandPhrase('role_create', 'ok'), 'Criou o papel')
  assert.equal(commandPhrase('role_create', 'failed'), 'Tentou criar o papel')

  // Ordem direta: create_agent
  assert.equal(commandPhrase('create_agent', 'running'), 'Criando o agente')
  assert.equal(commandPhrase('create_agent', 'ok'), 'Criou o agente')
  assert.equal(commandPhrase('create_agent', 'failed'), 'Tentou criar o agente')

  // Outras combinações de verbos e substantivos
  assert.equal(commandPhrase('delete_channel', 'ok'), 'Removeu o canal')
  assert.equal(commandPhrase('delete_channel', 'failed'), 'Tentou remover o canal')
  assert.equal(commandPhrase('update_backlog_item', 'ok'), 'Atualizou o card')
  assert.equal(commandPhrase('save_item_plan', 'ok'), 'Salvou o plano')
  assert(commandPhrase('role_list', 'ok'), 'Consultou o papel')
  assert.equal(commandPhrase('role_list', 'ok'), 'Consultou o papel')
  assert.equal(commandPhrase('set_setting', 'ok'), 'Definiu a configuração')
  assert.equal(commandPhrase('create_artifact', 'ok'), 'Criou o artefato')
})

test('commandPhrase — Overrides específicos preservam frases dedicadas', () => {
  assert.equal(
    commandPhrase('set_agent_instructions', 'running'),
    'Definindo instruções do agente',
  )
  assert.equal(
    commandPhrase('set_agent_instructions', 'ok'),
    'Definiu instruções do agente',
  )
  assert.equal(
    commandPhrase('set_agent_instructions', 'failed'),
    'Tentou definir instruções do agente',
  )

  assert.equal(commandPhrase('send_request', 'ok'), 'Enviou pedido')
  assert.equal(commandPhrase('send_request', 'failed'), 'Tentou enviar pedido')

  assert.equal(commandPhrase('answer_decision', 'ok'), 'Respondeu decisão')
  assert.equal(commandPhrase('answer_decision', 'failed'), 'Tentou responder decisão')

  assert.equal(commandPhrase('place_agent', 'ok'), 'Posicionou o agente')
  assert.equal(commandPhrase('place_agent', 'failed'), 'Tentou posicionar o agente')

  assert.equal(commandPhrase('reorder_layers', 'ok'), 'Reordenou as camadas')
  assert.equal(commandPhrase('reorder_layers', 'failed'), 'Tentou reordenar as camadas')
})

test('commandPhrase — Comandos fora das regras de ação/entidade caem no fallback genérico', () => {
  assert.equal(commandPhrase('comando_aleatorio_xyz', 'running'), 'Usando comando')
  assert.equal(commandPhrase('comando_aleatorio_xyz', 'ok'), 'Usou comando')
  assert.equal(commandPhrase('comando_aleatorio_xyz', 'failed'), 'Tentou usar comando')
  assert.equal(commandPhrase('foo', 'ok'), 'Usou comando')
})

test('friendlyCommand — Trunca nomes de comandos muito longos em 80 caracteres', () => {
  assert.equal(friendlyCommand('role_create'), 'role_create')
  const longo = 'a'.repeat(100)
  assert.equal(friendlyCommand(longo), 'a'.repeat(80))
})

// -------------------------------------------------------------
// Testes de activityRows com ToolDetail::AppCommand (kind: 'command')
// -------------------------------------------------------------
const mockCardTitleOf = (id) => (id === 'card-1' ? 'Implementar tela' : undefined)
const mockAgentNameOf = (id) => (id === 'hefesto' ? 'Hefesto (Arquiteto)' : undefined)

test('activityRows — Linha de comando com sucesso mapeia ícone de domínio e chip com subject', () => {
  const tool = {
    callId: 'call-1',
    name: 'mcp__vokia__app_command_call',
    category: 'app',
    target: 'role_create',
    status: 'ok',
    detail: {
      kind: 'command',
      command: 'role_create',
      domain: 'roles',
      subject: 'Tester',
    },
  }

  const rows = activityRows([tool], mockCardTitleOf, mockAgentNameOf)
  assert.equal(rows.length, 1)
  const row = rows[0]
  assert.equal(row.id, 'call-1')
  assert.equal(row.icon, 'role')
  assert.equal(row.verb, 'Criou o papel')
  assert.deepEqual(row.chip, {
    label: 'Tester',
    title: 'role_create',
    icon: 'generic',
  })
  assert.equal(row.failed, false)
})

test('activityRows — Linha de comando com falha (is_error) mantém detalhe e marca failed=true', () => {
  const tool = {
    callId: 'call-2',
    name: 'mcp__vokia__app_command_call',
    category: 'app',
    target: 'delete_channel',
    status: 'failed',
    detail: {
      kind: 'command',
      command: 'delete_channel',
      domain: 'channels',
      subject: 'geral',
    },
  }

  const rows = activityRows([tool], mockCardTitleOf, mockAgentNameOf)
  assert.equal(rows.length, 1)
  const row = rows[0]
  assert.equal(row.id, 'call-2')
  assert.equal(row.icon, 'channel')
  assert.equal(row.verb, 'Tentou remover o canal')
  assert.deepEqual(row.chip, {
    label: 'geral',
    title: 'delete_channel',
    icon: 'generic',
  })
  assert.equal(row.failed, true)
})

test('activityRows — Comando com agent no domínio ou no nome resolve nome amigável via nameOf', () => {
  const tool = {
    callId: 'call-3',
    name: 'mcp__vokia__app_command_call',
    category: 'app',
    target: 'set_agent_instructions',
    status: 'ok',
    detail: {
      kind: 'command',
      command: 'set_agent_instructions',
      domain: 'agents',
      subject: 'hefesto',
    },
  }

  const rows = activityRows([tool], mockCardTitleOf, mockAgentNameOf)
  assert.equal(rows.length, 1)
  const row = rows[0]
  assert.equal(row.icon, 'agent')
  assert.equal(row.verb, 'Definiu instruções do agente')
  assert.equal(row.chip?.label, 'Hefesto (Arquiteto)')
  assert.equal(row.chip?.title, 'set_agent_instructions')

  // Se o agente não está no elenco conhecido, mantém o id original
  const toolDesconhecido = {
    ...tool,
    callId: 'call-3b',
    detail: { ...tool.detail, subject: 'agente_desconhecido' },
  }
  const rows2 = activityRows([toolDesconhecido], mockCardTitleOf, mockAgentNameOf)
  assert.equal(rows2[0].chip?.label, 'agente_desconhecido')
})

test('activityRows — Comando sem subject usa o nome do comando como chip label', () => {
  const tool = {
    callId: 'call-4',
    name: 'mcp__vokia__app_command_call',
    category: 'app',
    target: 'select_project',
    status: 'ok',
    detail: {
      kind: 'command',
      command: 'select_project',
      domain: 'projects',
      subject: undefined,
    },
  }

  const rows = activityRows([tool], mockCardTitleOf, mockAgentNameOf)
  assert.equal(rows.length, 1)
  const row = rows[0]
  assert.equal(row.icon, 'project')
  assert.equal(row.chip?.label, 'select_project')
  assert.equal(row.chip?.title, 'select_project')
})

test('activityRows — Domínio desconhecido cai no ícone genérico "other"', () => {
  const tool = {
    callId: 'call-5',
    name: 'mcp__vokia__app_command_call',
    category: 'app',
    target: 'novo_modulo',
    status: 'ok',
    detail: {
      kind: 'command',
      command: 'novo_modulo',
      domain: 'dominio_inexistente',
      subject: 'teste',
    },
  }

  const rows = activityRows([tool], mockCardTitleOf, mockAgentNameOf)
  assert.equal(rows[0].icon, 'other')
})

test('activityRows — Suporte a todos os domínios mapeados em DOMAIN_ICON', () => {
  const dominios = [
    ['team', 'team'],
    ['projects', 'project'],
    ['channels', 'channel'],
    ['backlog', 'backlog'],
    ['artifacts', 'artifact'],
    ['attachments', 'attachment'],
    ['agents', 'agent'],
    ['roles', 'role'],
    ['orchestration', 'flow'],
    ['providers', 'provider'],
    ['settings', 'settings'],
  ]

  for (const [domain, expectedIcon] of dominios) {
    const tool = {
      callId: `call-${domain}`,
      name: 'mcp__vokia__app_command_call',
      category: 'app',
      target: 'cmd',
      status: 'ok',
      detail: {
        kind: 'command',
        command: 'cmd',
        domain,
        subject: 'sub',
      },
    }
    const [row] = activityRows([tool], mockCardTitleOf, mockAgentNameOf)
    assert.equal(row.icon, expectedIcon, `Ícone incorreto para domínio ${domain}`)
  }
})

// -------------------------------------------------------------
// Testes de activityClauses e activitySentence
// -------------------------------------------------------------
test('activitySentence e activityClauses — Agrupamento e contagem de ações no Vokia', () => {
  const tool1 = {
    callId: 'c1',
    name: 'mcp__vokia__app_command_call',
    category: 'app',
    target: 'role_create',
    status: 'ok',
  }
  const tool2 = {
    callId: 'c2',
    name: 'mcp__vokia__app_command_call',
    category: 'app',
    target: 'create_agent',
    status: 'ok',
  }
  const readTool = {
    callId: 'c3',
    name: 'Read',
    category: 'read',
    target: 'src/main.rs',
    status: 'ok',
  }

  assert.deepEqual(activityClauses([tool1]), ['executou 1 ação no Vokia'])
  assert.deepEqual(activityClauses([tool1, tool2]), ['executou 2 ações no Vokia'])
  assert.equal(
    activitySentence([readTool, tool1, tool2]),
    'Leu 1 arquivo e executou 2 ações no Vokia',
  )
})

// -------------------------------------------------------------
// Verificação final
// -------------------------------------------------------------
console.log(`\n=== Resumo dos Testes CMD-ACT-3 ===`)
const passedCount = results.filter((r) => r.passed).length
const failedCount = results.filter((r) => !r.passed).length
console.log(`Total: ${results.length} | Passaram: ${passedCount} | Falharam: ${failedCount}`)

if (failedCount > 0) {
  process.exit(1)
} else {
  console.log('Todos os testes de comando na atividade passaram com sucesso!')
}
