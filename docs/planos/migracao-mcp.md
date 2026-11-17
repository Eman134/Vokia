# Plano - Migracao para MCP do Vokia

## Estado
Rascunho completo, **MCP-0 concluída** (resultados na seção 6), **aguardando validação do usuário via Maestro**. Escrito em 2026-11-17 sobre o HEAD `8e9ec26` (BP-4 commitada). Raiz `C:/Users/Kayke/Documents/Projetos/Vokia/desktop`. **Mexe em `src-tauri`: sim. Mexe em `orchestrator/`: sim (MCP-4, MCP-6 a MCP-8, em série, R1).** As etapas abaixo trazem contrato, aceite, falhas e dependências; a nota literal com âncoras `caminho:linha` de cada etapa é escrita quando ela for a próxima (as âncoras de `turns.rs` e `agent_session.rs` mudam a cada etapa).

## Objetivo
- **WHY:** hoje o agente fala com o app por linhas de texto (`#card`, `#decisao`, `$var`, `@Nome:`). Cada marcador tem parser próprio, o erro só volta no turno seguinte (`REJECTION_PREFIX`, `turns.rs`), o agente não consegue ler o estado (não vê o backlog) e o prompt carrega uma regra por marcador (`prompts.rs:10-13`).
- **WHAT:** um servidor MCP embutido no Vokia, entregue junto com o app, que o app liga sozinho em toda sessão de agente que sobe. Os marcadores viram ferramentas tipadas.
- **HOW:** servidor HTTP local no próprio processo do app, autenticado por token por sessão, que sabe qual agente, projeto e troca fez cada chamada. O app gera a configuração e a passa ao `claude` em cada lançamento (`--mcp-config`).

## 1. Por que MCP
- **Tipado:** a CLI valida os argumentos contra o schema antes de chamar (a coluna `Ready` nem aparece no `enum` de `move_card`); o parser de texto some.
- **Valida por chamada:** o erro volta ao agente **na mesma resposta** (`isError`), e ele corrige sem gastar um turno de recusa nem contar em `rejections`.
- **O agente lê o estado:** `list_backlog` e `read_card` deixam o agente ver os cards antes de criar (hoje ele só descobre o duplicado depois, `ensure_title_free`).
- **Uma regra só:** `CARD_RULE`, `DECISION_RULE`, `PROMPT_RULES` e a parte de delegação viram uma frase curta ("use as ferramentas do Vokia") porque cada ferramenta se descreve sozinha.
- **Sem texto no feed:** a linha de marcador nunca vaza para a conversa; hoje depende do parser tirá-la (`parse_reply`).

## 2. Arquitetura
```
claude (sessão viva do agente)  --HTTP 127.0.0.1:<porta>/mcp, Bearer <token>-->  servidor MCP (dentro do app)
                                                                                    |-- registro token -> Binding
                                                                                    '-- Db, AppState.cache, AppHandle
```
- **Servidor no próprio app** (módulo novo `src-tauri/src/mcp/`), subido em `main.rs:70` (`setup`), porta efêmera em `127.0.0.1` (bind na porta 0), guardado como estado Tauri. Sem processo auxiliar nem sidecar.
- **Protocolo:** MCP "streamable HTTP" com resposta JSON simples a `POST /mcp` (JSON-RPC 2.0): `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call`. `GET /mcp` responde 405.
- **Token por sessão:** gerado a cada `LiveSession::open` (32 bytes aleatórios do SO, em hexadecimal). O registro guarda `token -> Binding { project_id, agent, agent_name, slot }`. Revogado quando a sessão fecha (`LiveSession::close`) ou o processo cai.
- **`ExchangeSlot`** (`Arc<Mutex<…>>` dentro do `Binding`): o que muda a cada troca numa sessão viva (a sessão atravessa várias trocas): `item_id`, `channel_id`, `confirmations: Vec<String>`, `changed: bool`, e depois `decision` e `delegation`. O orquestrador preenche antes de cada `send` e esvazia depois (`agent_session.rs`, `turns.rs`). É assim que a ferramenta conhece "o card da troca atual" (D4) sem o agente informar.
- **Configuração gerada por lançamento:** arquivo `<app data>/mcp/<uuid>.json` com `{"mcpServers":{"vokia":{"type":"http","url":"http://127.0.0.1:<porta>/mcp","headers":{"Authorization":"Bearer <token>"}}}}`; passado como `--mcp-config <arquivo> --strict-mcp-config` (o `--strict` ignora MCP do usuário: o agente só vê o `vokia`). Apagado ao fechar a sessão; a pasta é varrida na subida do app.
- **Só `Access::Project`** (sessão viva com projeto). `Access::TextOnly` (`claude/ask.rs`, `--tools ""`) não recebe MCP.
- **Permissão da CLI (M14):** `--allowedTools` ganha, **por nome completo**, `mcp__vokia__<ferramenta>` para cada ferramenta registrada (v1: `mcp__vokia__list_backlog`, `mcp__vokia__read_card`, `mcp__vokia__create_card`, `mcp__vokia__move_card`), em **todos** os níveis, inclusive Leitura: criar card não altera o projeto em disco, é estado do app. A lista sai do registro de ferramentas do servidor (uma fonte só); ferramenta nova entra sozinha. O curinga `mcp__vokia` **não** é usado. O `--permission-prompt-tool` (`live_session.rs:57`) **não** é consultado para essas ferramentas: o app as libera por `--allowedTools` e o `permission_denials` do resultado vem vazio (M15).
- O console já reconhece chamadas `mcp__` (`claude/activity.rs:22`); confirmar na MCP-5 que a linha da ferramenta aparece no Terminal.

## 3. Ferramentas
**v1 (backlog; MCP-3):**
- `list_backlog(column?: "Backlog"|"Ready"|"In progress"|"In review"|"Done")` → até 50 cards do projeto da sessão: `id, título, coluna, dono, origem` (uma linha por card). Mais de 50: devolve os 50 mais recentes e diz quantos ficaram de fora.
- `read_card(id)` → título, coluna, dono, origem, descrição, últimas 5 atividades.
- `create_card(title, description?)` → mesma regra de `create_agent_card` (`db/backlog.rs:642`): nasce em `Backlog`, `task`, origem `agente`; duplicado recusado.
- `move_card(id, column: "Backlog"|"In progress"|"In review"|"Done")` → regras de `move_agent_card` (D2 sem `Ready`, D4 só card próprio ou da troca, `Done` não volta).
Todas chamam o que a BP-1 já deixou no banco (`apply_card_operations`, `db/backlog.rs:356`); o projeto vem do `Binding`, nunca do argumento.

**v2 (fazem sentido, depois do corte do `#card`):**
- `request_decision(kind, title, detail?, options?)` (substitui `#decisao`). **Não bloqueia:** registra a decisão no `slot` e responde "Decisão registrada. Termine a resposta agora." O turno acaba e o orquestrador sobe a decisão pelo caminho que já existe (`raise_decision`, `turns.rs`). Bloquear a chamada até o usuário responder prenderia a sessão por minutos ou horas e estouraria o tempo da CLI.
- `get_variables()` e `set_variable(name, op: "set"|"append", value)` (substitui `$var`): lê e grava o `Cache` do projeto (`task/cache.rs`); só existem quando a troca tem `TaskDefinition` com variáveis. Os efeitos (`created_variable`, `changed_value`, aceitos) ficam no `slot` para o `GoalTracker` e a `feed_text` continuarem como hoje.
- `delegate(agent, request)` (substitui `@Nome:`): registra a delegação no `slot` e responde "Delegação registrada. Termine a resposta." O `next_turn` lê do `slot`.

## 4. Mapeamento e migração dos marcadores (decisão registrada)
| Marcador | Ferramenta | Migração |
|---|---|---|
| `#card criar/mover` | `create_card`, `move_card` | **Corte limpo** na MCP-4. A BP-4 (`8e9ec26`) não saiu em versão: `git tag --contains 8e9ec26` vem vazio e a última tag é `v0.3.0`. Remove `parse_card_line`, `cards` em `ParsedReply`, `apply_cards`, `MAX_CARD_OPERATIONS` e `CARD_RULE`. |
| `#decisao` | `request_decision` | **Compatibilidade por uma versão**: o parser continua aceitando a linha, o prompt só ensina a ferramenta. Removido na MCP-9. |
| `$var` | `get_variables`, `set_variable` | Igual: compatibilidade por uma versão, remoção na MCP-9. |
| `@Nome:` | `delegate` | Igual. A menção do usuário (`@Nome` na barra) **não** muda: é entrada do usuário, não marcador do agente. |
Motivo da compatibilidade nos três: sessões retomadas (`--resume`) têm histórico com o marcador antigo e o modelo o imita; o parser lenient absorve isso sem recusa.

## 5. Decisões (todas registradas; nenhuma pergunta ao usuário)
- **M1 Transporte:** HTTP local dentro do app. Descartado o processo auxiliar por stdio (segundo executável, empacotamento, canal de volta ao app, ciclo de vida no Windows).
- **M2 Dependência:** `axum` para o roteamento HTTP (já há `tokio`; `tauri` traz `hyper`). Descartada a crate `rmcp` (API ainda muda) e o servidor HTTP escrito à mão. O protocolo MCP é escrito no projeto (cinco métodos, JSON-RPC pequeno) em `mcp/protocol.rs`. A MCP-1 abre com `cargo add axum` (versão estável atual) e `cargo tree -d` sem duplicar `hyper`/`tokio`.
- **M3 Segurança:** só `127.0.0.1`; `Authorization: Bearer` obrigatório, comparação em tempo constante; cabeçalho `Host` precisa ser `127.0.0.1:<porta>` (protege de DNS rebinding); `Origin` presente é recusado; token novo a cada `open`, revogado no `close`; token nunca em log, evento, console nem na linha de comando (por isso arquivo e não JSON inline em `--mcp-config`).
- **M4 Identidade:** agente, projeto e troca vêm só do `Binding`/`slot`; nenhuma ferramenta aceita `project_id` nem `agent` como argumento.
- **M5 Erro de ferramenta:** resposta `isError: true` com a mesma frase em português de `DbError` (`db/error.rs:144-157`); não conta em `rejections`; o teto é o `--max-turns` da CLI (`command.rs:155`). Erro de sistema (banco indisponível) vira `isError` genérico e vai ao feed por `reportError`.
- **M6 Confirmação para o usuário (D7b):** continua. A ferramenta guarda `Card <id> criado: <título>.` no `slot`; `run_turns` anexa ao fim da mensagem do agente. O evento `backlog-changed` sai do handler, depois da gravação (o front da BP-5 não muda).
- **M7 Servidor fora do ar:** se o bind falha na subida, o app mostra o erro uma vez no feed e os agentes sobem **sem** `--mcp-config` e sem a regra de ferramentas (backlog por agente indisponível). Sem retorno silencioso aos marcadores novos.
- **M8 Proxy:** `base_command` ganha `NO_PROXY=127.0.0.1,localhost` (proxy corporativo não pode interceptar o loopback).
- **M9 Ordem dos flags:** `--mcp-config` é variádico; fica logo antes de `--append-system-prompt`, com `--strict-mcp-config` imediatamente depois (como já está comentado em `command.rs:128-130`).
- **M10 Custo de tokens:** no máximo 4 ferramentas na v1 e 8 ao fim, descrições de uma frase. A regra de prompt cai de 3 parágrafos para 1 frase. A MCP-5 mede o total de tokens de entrada de um turno antes e depois (`usage` do stream) e reporta; se subir mais de 10%, enxugar as descrições.
- **M11 Leitura em todos os níveis:** `list_backlog` e `read_card` valem em `Leitura`, `Edição` e `Liberado`.
- **M12 Testes Rust:** o corpo fica em `src-tauri/tests/<nome>_tests.rs`; no fonte só a linha `#[cfg(test)] #[path = "../../tests/<nome>_tests.rs"] mod tests;` (padrão de `claude/command.rs:171`). Nunca código de teste dentro de `src/`. Testes HTTP usam `tokio::net::TcpStream` (sem dependência nova de cliente).
- **M13 Ordem do E2E:** o fake-claude (`e2e/fake-claude/claude.mjs`) ganha a leitura de `--mcp-config` e o comando "ferramenta <nome> <json>", que chama o servidor de verdade por HTTP. O E2E exercita o servidor real, não um mock.
- **M14 Liberação de ferramentas (decidido após a MCP-0):** por nome completo `mcp__vokia__<ferramenta>` em `--allowedTools`, gerado pelo app a partir do registro de ferramentas; sem curinga.
- **M15 Sem `--permission-prompt-tool` para o Vokia:** o canal de permissão do usuário (`live_session.rs:57`) continua só para Bash e arquivos; ferramenta do Vokia nunca pergunta ao usuário, porque o app já a libera e valida por chamada.
- **M16 Métodos desconhecidos:** o servidor responde `-32601` a qualquer método que não seja `initialize`, `notifications/initialized`, `ping`, `tools/list` e `tools/call` (inclusive `server/discover`), e `405` a `GET /mcp`. A CLI tolera isso e segue o handshake.
- **M17 Volta extra do `ToolSearch`:** a CLI chama `ToolSearch` antes da primeira ferramenta MCP de cada sessão (uma volta a mais). O desenho não muda; o `--max-turns` (`command.rs:155`, 40) comporta; a medição de tokens da MCP-5 inclui a volta. Se um dia pesar, a descrição curta das ferramentas é a alavanca (M10), não o desenho.

## 6. Etapas (ordem única)
| Ordem | Id | Papel | Nível | Arquivos | Depende de |
|---|---|---|---|---|---|
| 1 | MCP-0 | ✅ **Concluída** (Implementador, só leitura) | Básico | nenhum | — |
| 2 | MCP-1 | Implementador | Intermediário | `Cargo.toml`, `mcp/{mod,server,protocol,registry}.rs` (novos), `main.rs`, `tests/mcp_server_tests.rs` | MCP-0 |
| 3 | MCP-2 | Implementador | Intermediário | `claude/command.rs`, `claude/live_session.rs`, `orchestrator/agent_session.rs`, `tests/command_tests.rs`, `e2e/fake-claude/claude.mjs` | MCP-1 |
| 4 | MCP-3 | Implementador | Intermediário | `mcp/tools.rs` e `mcp/slot.rs` (novos), `db/backlog.rs` (`read_card`), `tests/mcp_backlog_tests.rs` | MCP-1 |
| 5 | MCP-4 | Implementador | Intermediário | `orchestrator/{markers,turns,prompts}.rs`, `tests/` do `#card` | MCP-2, MCP-3 |
| 6 | MCP-5 | Tester | — | `e2e/mcp-backlog.cdp.mjs` e `-rodar.sh` (novos) | MCP-4 e BP-5 |
| 7 | MCP-6 | Implementador | Intermediário | `mcp/tools.rs`, `orchestrator/turns.rs`, `prompts.rs` | MCP-5 |
| 8 | MCP-7 | Implementador | Intermediário (esboço) | `mcp/tools.rs`, `task/cache.rs`, `turns.rs` | MCP-6 |
| 9 | MCP-8 | Implementador | Básico (esboço) | `mcp/tools.rs`, `turns.rs` | MCP-7 |
| 10 | MCP-9 | Implementador | Básico | `markers.rs`, `turns.rs`, `prompts.rs`, `task/cache.rs` | MCP-8 e uma versão publicada |

A **MCP-5 substitui a BP-6** do plano de backlog (a BP-6 nunca foi escrita; o E2E dela passa a testar a ferramenta, não o marcador). A **BP-5** (front recarrega no evento) segue como está e é pré-requisito da MCP-5. MCP-2 e MCP-3 tocam arquivos disjuntos e podem andar em paralelo depois da MCP-1; MCP-4 e MCP-6 a MCP-9 mexem em `orchestrator/` e vão em série, sem outra etapa de orquestrador junto.
Commits propostos: MCP-1 `feat(mcp): servidor MCP local do app`; MCP-2 `feat(claude): configura o MCP em cada sessão`; MCP-3 `feat(mcp): ferramentas de backlog`; MCP-4 `refactor(orchestrator): #card vira ferramenta MCP`.

### MCP-0 · Verificar a CLI (só leitura) — ✅ CONCLUÍDA
Executada pelo Implementador; **não repetir a chamada real**. Evidência registrada pelo Maestro:
- (a) A CLI aceita `--mcp-config` com `"type":"http"` e `headers`, junto com `--strict-mcp-config`.
- (b) O servidor HTTP local recebeu `Authorization: Bearer <token>`.
- Handshake observado, na ordem: `server/discover` (a CLI tenta primeiro), `initialize`, `notifications/initialized`, `tools/list`.
- (c) Com `--allowedTools mcp__vokia__echo`, o `tool_use` `mcp__vokia__echo` rodou com `permission_denials` vazio. Liberação por **nome completo** (M14).
- (d) O nome chega como `mcp__vokia__<ferramenta>`.
- (e) `--permission-prompt-tool` não é necessário para as ferramentas do Vokia (M15).
- Observação: a CLI chama `ToolSearch` antes da primeira ferramenta MCP (uma volta extra, M17).
- **Efeito no plano:** nenhum bloqueio; M1 (HTTP) confirmada; M14 a M17 registradas; MCP-1 responde `-32601` a `server/discover`.

### MCP-1 · Servidor MCP local
- **Contrato:** módulo `mcp` com `McpServer::start(app) -> Result<McpServer, McpError>` (porta 0 em `127.0.0.1`), `register(binding) -> Token`, `revoke(&Token)`, `url()`. Rotas: `POST /mcp`. Métodos: `initialize` (devolve `protocolVersion`, `capabilities.tools`, `serverInfo {name:"vokia"}`), `notifications/initialized` (204), `ping`, `tools/list` (lista do registro de ferramentas; vazia nesta etapa), `tools/call` (ferramenta desconhecida → erro JSON-RPC `-32602`). **Qualquer outro método (inclusive `server/discover`, que a CLI envia no início) responde `-32601` (M16).** Autenticação e validação de `Host`/`Origin` conforme M3. Estado Tauri: `McpState` gerenciado em `main.rs:70` (falha de bind guardada, não derruba o app, M7).
- **Aceite:** testes em `tests/mcp_server_tests.rs`: sem token → 401; token errado → 401; token revogado → 401; `Host` errado → 403; `initialize` e `tools/list` respondem JSON-RPC válido; `GET /mcp` → 405 (M16); `server/discover` → `-32601` e a CLI segue para `initialize`; corpo maior que 64 KB → 413; bind em porta 0 devolve porta > 0 e só ouve em loopback.
- **Falhas a cobrir:** token em branco; `Bearer` com maiúscula/minúscula errada; dois tokens de agentes diferentes não se cruzam; revogar durante uma chamada em andamento; JSON inválido → `-32700`; requisição sem `method` → `-32600` (JSON-RPC: requisição inválida, não método inexistente); chamada concorrente de dois agentes.

### MCP-2 · Configuração em cada lançamento
- **Contrato:** `agent_command` (`claude/command.rs:119`) recebe `mcp: Option<&McpConfigFile>` e, só em `Access::Project`, acrescenta `--mcp-config <caminho> --strict-mcp-config` (antes de `--append-system-prompt`, M9) e os nomes completos `mcp__vokia__<ferramenta>` em `--allowedTools` (`allowed_tools`, `:171`, M14). `LiveSession::open` (`live_session.rs:40`) ganha o parâmetro; `agent_session.rs:171` (`fresh_reply`) e `:376` (sessão reutilizada) criam `Binding` + token + arquivo antes do `open` e revogam e apagam depois do `close`. `base_command` ganha `NO_PROXY` (M8). O `ExchangeSlot` é preenchido com `item_id` e `channel_id` antes de cada `send` e esvaziado depois.
- **Aceite:** `tests/command_tests.rs`: com MCP, `--mcp-config` aparece antes de `--append-system-prompt`, `--strict-mcp-config` logo após, cada `mcp__vokia__<ferramenta>` está em `--allowedTools` nos três níveis e `mcp__vokia` sozinho não aparece; sem MCP ou em `TextOnly`, nada disso aparece. O fake-claude lê o arquivo e consegue chamar `initialize` com o token.
- **Falhas:** arquivo sobra depois de crash (varredura na subida); token vazando para `--append-system-prompt` ou para o console; dois lançamentos do mesmo agente reaproveitando o token; `--resume` com o token antigo (precisa de token novo); caminho com espaço no Windows.

### MCP-3 · Ferramentas de backlog
- **Contrato:** `mcp/tools.rs` registra `list_backlog`, `read_card`, `create_card`, `move_card` com schema (JSON Schema, `enum` de colunas sem `Ready` em `move_card`, `additionalProperties:false`). `Db::read_card(project_id, id)` novo (público, usa `find_card`, `db/backlog.rs:632`). `create_card`/`move_card` chamam `apply_card_operations` com `AgentActor` montado do `Binding` (`current_card` = `slot.item_id`); sucesso grava confirmação no `slot` e emite `backlog-changed` (`emit_backlog_changed`) só se algo mudou; `DbError::is_card_refusal` decide entre `isError` com a frase do banco e erro de sistema.
- **Aceite:** `tests/mcp_backlog_tests.rs` com banco em memória e servidor real: criar, mover, ler, listar; as mesmas recusas da BP-4 (Ready, card alheio, id inexistente, título duplicado, `Done` não volta) como `isError`.
- **Falhas:** `project_id` ou `agent` nos argumentos ignorados (injeção: o teste manda e confere que não vale); título gigante (limite 200 caracteres, decisão aqui: recusa com frase); `list_backlog` de projeto com 0 cards; coluna em minúscula (`in progress` aceita, o banco normaliza); card de outro projeto com o mesmo id.

### MCP-4 · Corte do `#card` e prompt
- **Contrato:** remover o que a seção 4 lista; `CARD_RULE` vira `TOOLS_RULE` (uma frase: "Use as ferramentas do Vokia para ler e mexer no backlog; nunca invente ids."), incluída em `role` só quando há MCP (M7). `run_turns` esvazia `slot.confirmations` e `slot.changed` depois de `ask_step` e anexa as confirmações ao texto (`with_confirmations`, hoje em `turns.rs`).
- **Aceite:** `cargo test` verde; `rg "#card" src-tauri/src` só acha comentário de migração (nenhum); mensagem do agente que usou `create_card` termina com `Card <id> criado: <título>.`.
- **Falhas:** linha `#card` escrita pelo agente por hábito vira texto comum visível (aceito, não executa); confirmação duplicada se o turno repetir; `changed` vazando de uma troca para a outra no `slot`; `raise_decision` com card criado no mesmo turno (o card fica, a decisão sobe).

### MCP-5 · E2E (Tester)
- **Contrato:** `e2e/mcp-backlog.cdp.mjs` + `mcp-backlog-rodar.sh` no padrão de `telas-agente-rodar.sh`; o fake-claude chama as ferramentas pelo servidor real (M13).
- **Aceite (≥ 10 cenários):** criar card por agente aparece no backlog aberto sem recarregar (BP-5); mover; `list_backlog` devolve o card criado; Ready, card alheio, id inexistente e duplicado → mensagem de erro na conversa e nada gravado; token errado → fake recebe 401 e o app segue vivo; projeto B não vê card do A; reabrir o app mantém origem e atividade; linha da ferramenta aparece no Terminal sem o token; **nenhum token no DOM, nos eventos nem nos logs**; medição de tokens de entrada antes e depois (M10), reportada.
- **Controles negativos (revertidos):** servidor sem checar token ⇒ cenário do 401 vermelho; `project_id` aceito do argumento ⇒ cenário do projeto B vermelho; `Ready` no `enum` ⇒ cenário Ready vermelho.

### MCP-6 a MCP-9 (esboço; nota literal quando for a vez)
- **MCP-6 `request_decision`:** o `slot.decision` é lido logo depois de `ask_step` e antes de `parse_decision` (`turns.rs`), subindo por `raise_decision`. Falhas: decisão registrada duas vezes no mesmo turno (a segunda é erro); tipo `escolha` sem opções (erro, igual a `decision_line`); resposta do usuário a uma decisão antiga depois de reabrir o app.
- **MCP-7 variáveis:** só depois de ler `Cache::apply_reply` e `AppliedReply` por inteiro (`task/cache.rs`); o `slot` acumula os efeitos para `GoalTracker`. É a etapa de maior risco (meta, parada e repetição dependem disso); se a leitura mostrar acoplamento além do esperado, volta ao Planejador para decidir antes de delegar.
- **MCP-8 `delegate`:** registra `(agente, pedido)` no `slot`; `next_turn` usa. Falhas: delegar a si mesmo, a agente inexistente, pedido vazio, duas delegações no mesmo turno.
- **MCP-9 remoção da compatibilidade:** remover `parse_decision`/`decision_line`, `parse_update_line` no caminho do agente, `delegation_request` e as constantes; só depois de uma versão publicada com as ferramentas. Falha: sessão retomada que ainda escreve o marcador antigo (vira texto comum).

## 7. Riscos
- **R1 Token:** qualquer processo do mesmo usuário lê o arquivo de configuração. Aceito: o servidor só ouve em loopback, o token morre com a sessão e o que ele pode fazer é o que o agente já podia (mexer em cards do projeto). Mitigação: arquivo apagado ao fechar, varredura na subida, nunca em log.
- **R2 Processo:** o servidor mora no processo do app; se o app fecha, as sessões fecham juntas (`close_all`, `main.rs:84`). Se o `claude` perde a conexão, a chamada falha com erro e o agente segue sem backlog, sem derrubar a troca.
- **R3 Windows:** firewall não avisa para loopback; antivírus pode atrasar o bind (tempo limite de 5 s na subida, falha vira M7); caminho do arquivo com espaços e `\` (testar na MCP-2); `NO_PROXY` (M8).
- **R4 Permissão da CLI:** resolvido pela MCP-0: liberando por nome completo em `--allowedTools`, a chamada roda sem pergunta e sem `permission_denials` (M14, M15). Resta o risco de uma ferramenta nova faltar na lista; por isso a lista sai do registro do servidor. Efeito colateral aceito: o `--strict-mcp-config` esconde os MCPs do usuário dos agentes do Vokia.
- **R5 Custo de tokens:** a definição das ferramentas entra em todo turno e a CLI faz uma volta extra de `ToolSearch` antes da primeira chamada de ferramenta MCP (M17); mitigado por M10 e medido na MCP-5 (a medição inclui essa volta).
- **R6 Sessão viva:** a sessão atravessa trocas; item e canal mudam entre elas. O `slot` é atualizado antes de cada `send`; esquecer isso faz o D4 usar o card da troca anterior (falha coberta na MCP-3 e na MCP-5).
- **R7 Versão da CLI:** a MCP-0 confirmou `--mcp-config` com `type:http` e `headers`, com `--strict-mcp-config`, na CLI instalada. O formato pode mudar em versões futuras: o app mostra no feed o erro que a CLI devolver ao recusar o flag (caminho de `ClaudeError`, como hoje), e a MCP-5 repete o handshake de ponta a ponta.
- **R8 Corrida front × agente (R1 do plano de backlog)** segue igual: o front grava o item inteiro; a BP-5 recarrega no evento.

## 8. O que não fazer
- Não criar processo auxiliar nem segundo executável; não usar stdio.
- Não expor o servidor fora do loopback nem sem token; não aceitar `project_id`/`agent` como argumento de ferramenta.
- Não passar o token na linha de comando, no prompt, no console, em evento ou em log.
- Não dar MCP a `Access::TextOnly`; não herdar MCP do usuário (sempre `--strict-mcp-config`).
- Não bloquear `request_decision` até o usuário responder.
- Não manter `#card` em compatibilidade (corte limpo) nem remover `#decisao`/`$var`/`@Nome:` antes da MCP-9.
- Não escrever teste dentro de `src/` (só a linha `#[path]`); não criar teste de componente no front.
- Não editar o front nesta migração (o evento `backlog-changed` já é da BP-5).
- Não rodar a CLI real para repetir a MCP-0; o E2E usa o fake-claude (M13).
- Não usar o curinga `mcp__vokia` em `--allowedTools` nem consultar `--permission-prompt-tool` para ferramentas do Vokia (M14, M15).

## 9. Perguntas em aberto
Nenhuma. M1 a M17 decididas; a MCP-0 confirmou o que dependia da CLI (HTTP, `headers`, `--strict-mcp-config`, liberação por nome completo).

## 10. Resultado
- **MCP-9:** feita antes da versão publicada (decisão aceita).
- **MCP-10:** fallback sem MCP em `c14d2c7`.
- **MCP-11:** `update_card` em `4766f4a`; card em Ready é recusado.
- **Delegação automática:** `@Nome:` no texto delega, e a resposta de quem foi chamado aparece como fala dirigida a quem pediu, sem novo turno.
- **Dívida aceita:** controles negativos de sub-abas, canais e MC-1.
