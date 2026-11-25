# Pesquisa · OpenBot e Orquestradores de IA para Desenvolvimento

> **Data:** 2026-11-25  
> **Objetivo:** Levantar como o **OpenBot** (`nightly-labs/openbot`) e os principais orquestradores de IA voltados a desenvolvimento implementam conexão com provedores (**Claude, Codex, OpenCode e Gemini**), seletores de provedor/agente, buscas e pesquisas, estatísticas de uso, janelas de horas/semanais e rastreamento de etapas — traduzindo tudo em recomendações técnicas concretas para evoluir o **Vokia**.  
> **Relacionado:** [Visão de produto](vokia-visao-de-produto.md) · [Plano de melhorias](plano-de-melhorias.md) · [Plano UI/UX](plano-ui-ux.md)

---

## Sumário

1. [Diagnóstico do Vokia hoje vs. lacunas mapeadas](#1-diagnóstico-do-vokia-hoje-vs-lacunas-mapeadas)
2. [Análise profunda do OpenBot](#2-análise-profunda-do-openbot)
   - [2.1 Conexão e ciclo de vida dos 4 provedores](#21-conexão-e-ciclo-de-vida-dos-4-provedores)
   - [2.2 Seletor de provedores/modelos e seletor de agentes](#22-seletor-de-provedoresmodelos-e-seletor-de-agentes)
   - [2.3 Buscas e pesquisas (app, histórico, ferramentas e web/browser)](#23-buscas-e-pesquisas-app-histórico-ferramentas-e-webbrowser)
   - [2.4 Estatísticas de uso e analytics locais](#24-estatísticas-de-uso-e-analytics-locais)
   - [2.5 Janelas de 5 horas, semanais e controle de limites](#25-janelas-de-5-horas-semanais-e-controle-de-limites)
   - [2.6 Etapas, rastreamento e motor anti-abandono de tarefas](#26-etapas-rastreamento-e-motor-anti-abandono-de-tarefas)
   - [2.7 Outros diferenciais relevantes do OpenBot](#27-outros-diferenciais-relevantes-do-openbot)
3. [Levantamento completo de orquestradores de IA para desenvolvimento](#3-levantamento-completo-de-orquestradores-de-ia-para-desenvolvimento)
   - [3.1 Matriz comparativa geral](#31-matriz-comparativa-geral)
   - [3.2 Análise detalhada por orquestrador](#32-análise-detalhada-por-orquestrador)
   - [3.3 Padrões convergentes do mercado (2025–2026)](#33-padrões-convergentes-do-mercado-20252026)
4. [Plano técnico de adoção no Vokia](#4-plano-técnico-de-adoção-no-vokia)
   - [4.1 Arquitetura para os 4 provedores (Claude, Codex, OpenCode, Gemini)](#41-arquitetura-para-os-4-provedores-claude-codex-opencode-gemini)
   - [4.2 Seletor de modelos e seletor de agentes](#42-seletor-de-modelos-e-seletor-de-agentes)
   - [4.3 Motor de cotas multi-provedor (5h / Semanal / Mensal) e estatísticas](#43-motor-de-cotas-multi-provedor-5h--semanal--mensal-e-estatísticas)
   - [4.4 Rastreamento de etapas e anti-abandono por turno](#44-rastreamento-de-etapas-e-anti-abandono-por-turno)
   - [4.5 Busca global, descoberta de ferramentas e pesquisa](#45-busca-global-descoberta-de-ferramentas-e-pesquisa)
   - [4.6 Priorização sugerida por ondas](#46-priorização-sugerida-por-ondas)
5. [Fontes e referências](#5-fontes-e-referências)

---

## 1. Diagnóstico do Vokia hoje vs. lacunas mapeadas

Com base na leitura direta do código atual do Vokia (`desktop/src-tauri/src/` e `desktop/src/features/`):

| Dimensão | Estado atual no Vokia | Lacuna principal em relação ao objetivo |
|---|---|---|
| **Os 4 provedores** | `providers.rs` já declara 4 seeds: `claude` (`ClaudeLive`), `codex` (`CodexJsonl`), `opencode` (`OpenCodeJson`) e `antigravity` (`AntigravityStream`, comando `agy`), mais `GenericCli`. | 1) **Gemini** aparece como `Antigravity` (`agy`), sem opção/clareza entre Google Antigravity CLI (`agy`) e `@google/gemini-cli` (`gemini`);<br>2) **Codex** roda `codex exec --json` efêmero por turno (não usa `codex app-server` persistente, perdendo resume nativo, cache MCP e leitura de cotas);<br>3) **OpenCode** roda `opencode run --format json` com `VokiaCli` (sem ACP, sem MCP nativo, bloqueia modo `Liberado`);<br>4) Não há fluxo de login/autenticação assistido nem download/gestão de runtimes das CLIs. |
| **Seletor de provedor/modelo e seletor de agente** | `ActiveLlmSwitcher.vue` já possui um modal com busca, lista agrupada por provedor, badges de tier (`Básico/Intermediário/Avançado`) e badge `Grátis`. | 1) Falta navegação em **abas/trilho por provedor** com logo, status real de conexão/autenticação, badge de **cota esgotada / renova em X** e sub-seletor de **reasoning effort** (`low/medium/high/max`);<br>2) Falta um **seletor unificado e pesquisável de agentes** (avatar, papel, provedor atual, fila/estado) estilo `AgentPicker` do OpenBot para direcionar pedidos, filtros e delegações. |
| **Estatísticas de uso e janelas (5h / semanal)** | `claude/stream.rs` extrai `rate_limit_event` (`unifiedWindows.five_hour` e `seven_day`) e `TurnUsage`. `codex.rs`, `opencode.rs` e `antigravity.rs` retornam **`limits: None`**. `usagePlans.ts` e `useUsage.ts` estão **hardcoded apenas para `cc` (Claude Code)**. | 1) Zero visibilidade de janelas de 5h / semanal para **Codex**, **OpenCode Go** e **Gemini**;<br>2) Sem polling controlado (`UsageReadGate` com cache + backoff exponencial contra 429);<br>3) Sem indicador de **ritmo (pace)** comparando cota restante vs. tempo restante da janela;<br>4) Sem histórico desacoplado das conversas (com agregação diária por provedor, modelo e agente + distinção de `cache_write` vs. `uncached_input`). |
| **Etapas e rastreamento (não perder a tarefa)** | Vokia tem um diferencial fortíssimo de **macro-orquestração** (`plan_stages.rs`, kanban, lotes, dependências `plan_dependencies`, `goal.rs`, `loop_guard.rs`). | Falta a camada de **micro-rastreamento dentro do turno e entre turnos**:<br>1) Não captura nem exibe em tempo real o plano interno vivo do agente (`TodoWrite`/`TaskUpdate` do Claude, `turn/plan/updated` do Codex, `plan` do ACP);<br>2) Não existe motor de **continuação automática** quando o agente para no meio dizendo *"Vou agora rodar os testes..."* ou deixa etapas do plano abertas;<br>3) Não gera rótulos humanos legíveis por passo (`Lendo providers.rs`, `Buscando na web...`);<br>4) Não captura `[work steps]` compactos para **handoff** ao trocar de provedor ou rotacionar sessão. |
| **Buscas e pesquisas** | Filtros locais por tela e catálogo de comandos (`commands/list` + `commands/call`) no MCP. | Falta **busca global (`⌘K`)** unificada (agentes, projetos, cards, mensagens, arquivos, configurações), **busca de histórico pela IA** (`history_search` / `history_read`), **descoberta de ferramentas por busca** (`tool_search`) e estratégia clara de **pesquisa web** para provedores que não têm web search nativa. |

---

## 2. Análise profunda do OpenBot

O **OpenBot** (`nightly-labs/openbot`, site `openbot.run`) é um workspace desktop/web/mobile local-first (monorepo Bun + Electron + SolidJS + Effect + SQLite) feito para operar times persistentes de agentes de IA usando as assinaturas e CLIs que o usuário já possui. Diferente do Vokia, o OpenBot **não** é focado exclusivamente em engenharia de software (ele cobre trabalho geral, browser, Computer Use, canais, rotinas, WhatsApp/Slack/Discord/Telegram e workflows visuais), mas sua engenharia de **infraestrutura de provedores, cotas, busca e confiabilidade de turnos** é hoje uma das mais maduras em código aberto.

### 2.1 Conexão e ciclo de vida dos 4 provedores

No OpenBot, todo provedor é modelado em duas camadas desacopladas:
1. **Descriptor declarativo** (`packages/contracts/src/agent-providers.ts`): define `id`, `displayName`, `cliName`, `authKind`, `pickerOrder`, `localCodeSignIn`, `skillFolders`, `workspaceInstructionFiles`, `workspaceEnforcement`, `steer` (`native | deferred | reported`), `concurrentTurns` e `reportsUsage`.
2. **Driver de execução** (`src/backend/provider-drivers.ts`): implementa o protocolo real, descoberta de modelos, autenticação, isolamento de processo, leitura de cotas e atualização de binário.

#### Protocolos usados para cada um dos 4 provedores

| Provedor | Protocolo no OpenBot | Como conecta e executa | Diferença chave para o Vokia hoje |
|---|---|---|---|
| **Claude** (`claude`) | **Claude Agent SDK / CLI live** (`claude-client.ts`) | Mantém sessão viva com setters em tempo real (`setModel`, `setPermissionMode`, `applyFlagSettings` para `effortLevel` e `fastMode`) **sem reiniciar a sessão**. Desativa skills e tools do host do Claude Code que não fazem sentido no orquestrador, reduzindo tokens de prompt. Usa `CLAUDE_CODE_TERMINAL_MCP_TOOLS` para encerrar turnos silenciosos sem texto redundante. | Vokia já usa `ClaudeLive`, mas ainda pode adotar troca de modelo/effort sem matar a sessão e filtro de tools/skills desnecessárias. |
| **Codex** (`codex`) | **Codex App Server RPC** (`codex app-server`) | Um único `app-server` por conta gerencia múltiplas threads via JSON-RPC (`thread/start`, `thread/resume`, `thread/unsubscribe`, `thread/archive`, `model/list`, `account/rateLimits/read`, `account/login/start`). Desativa `features.tool_suggest`, `features.recommended_plugins` e skills do Codex App (economiza ~1.050 tokens/request no GPT-6 Luna). | Vokia usa `codex exec --json` por turno (cold start toda vez, sem RPC de cota e sem resume rico). Migrar para `codex app-server` destrava cotas, resume instantâneo, cache MCP de 30 min e steering nativo. |
| **OpenCode** (`opencode`) | **ACP (`opencode acp`)** (`acp-client.ts` + `acp-flavor-opencode.ts`) | Usa o **Agent Client Protocol (ACP)** padronizado sobre stdio. Para evitar vazamento de processos MCP stdio no OpenCode 2.x, o OpenBot sobe um **relay local Streamable HTTP em `127.0.0.1` com bearer token** (`mcp-stdio-relay.ts`) compartilhado por diretório. Desativa tools redundantes (`question`, `opencode_*_mcp_resources`) via `OPENCODE_CONFIG`, economizando ~11,7% de tokens por turno. Suporta modelos **grátis sem conta** (`opencode/big-pickle`, etc.) e chaves **OpenCode Go**. | Vokia usa `opencode run --format json` + `VokiaCli`. Adotar `opencode acp` dá MCP nativo, steering (`_session/steering`), eventos estruturados de plano e suporte a subagentes em background (`unsentBackgroundTurns`). |
| **Gemini** (`antigravity`) | **ACP (`antigravity acp`)** (`acp-flavor-antigravity.ts`) | Usa a CLI oficial `google-antigravity/antigravity-cli` em modo ACP (exibida ao usuário como **"Gemini"** — Google AI Pro/Ultra ou `GEMINI_API_KEY`). Isola credenciais por conta com `--gemini-dir`, `--app-data-dir` e `WSL_DISTRO_NAME` para armazenar o OAuth em arquivo (`antigravity-oauth-token`) em vez de colidir no Keychain global. Suporta steering por segundo `session/prompt` (v1.5+). | Vokia já usa `agy` em `stream-json`, mas sem ACP, sem leitura de cota do Code Assist e sem expor "Gemini" de forma amigável. Além disso, o Vokia pode avaliar suportar **`gemini` (`@google/gemini-cli`)** como alternativa/modo API direta além do `agy` (planos Google AI Pro/Ultra). |

#### Padrões de conexão, autenticação e gestão de processos do OpenBot

- **Taxonomia de login (`docs/architecture/integrations.md`)**:
  - **Loopback callback** (`http://127.0.0.1:<porta>/...`): Codex browser sign-in e servidores MCP OAuth.
  - **Device code**: `codex login --device-auth` (usuário digita código curto no navegador; funciona até em servidores sem tela).
  - **Pasted code**: `claude auth login --claudeai` e `agy acp --login` (CLI imprime link, usuário cola código/URL de retorno; vai só por `stdin`, nunca em log).
  - **CLI-owned browser login**: `acp authenticate` (`oauth-personal` no Antigravity/Gemini) com interceptação segura de URL.
  - **API Key / Gateway por provedor**: variáveis por provedor em storage criptografado (`ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN`, `OPENAI_BASE_URL` via `-c model_provider`, `AGY_LLM_GATEWAY_URL`, `GEMINI_API_KEY`), permitindo inclusive usar proxies como `CLIProxyAPI`.
- **Resolução e gestão de runtimes (`provider-runtime-releases.ts`)**:
  - Ordem de precedência: variável explícita (`ENV`) → binário gerenciado baixado no store local (verificado por **SHA-256** contra GitHub releases/npm integrity + **blocklist remota** para barrar versões quebradas) → CLI do sistema no `PATH` (se igual ou mais nova, a do usuário vence).
  - Atualização segura: aguarda o provedor ficar sem turnos ativos antes de trocar o binário; se a nova versão falhar na verificação `--version`, mantém a anterior intacta.
- **Gestão de sessões ociosas (`IdleThreadPool` e `CodexIdleSessions`)**:
  - Uma sessão carregada segura processos da CLI + MCP + terminais. O OpenBot descarrega threads ociosas mantendo o `session_id` para retomar no próximo turno (Codex: libera thread após 5 min, mantendo o `app-server` por 30 min para preservar o cache da lista de tools MCP; Claude/ACP: fecha processo ocioso se não houver comando em background rodando).
  - Mata árvore de processos de forma limpa (`SIGKILL` no process group em Unix, `taskkill /T` no Windows).
- **Resiliência de catálogo de modelos (`moveAgentsOffUnlistedModels`)**:
  - Se uma atualização remove um modelo da lista de um provedor, move automaticamente para o mais novo da mesma família mantendo variante/esforço (ex.: `claude-opus-4-6-thinking` → `claude-opus-5-5-thinking`). No OpenCode (que carrega lista parcial no boot), espera 2 inícios seguidos sem o modelo antes de migrar.

---

### 2.2 Seletor de provedores/modelos e seletor de agentes

#### A. Seletor de Provedor e Modelo (`ProviderModelPicker.tsx` + `provider-model-options.ts`)

Diferente de uma lista corrida única, o seletor do OpenBot combina **navegação por provedor (trilho/abas)** + **busca rápida** + **metadados operacionais em tempo real**:

1. **Trilho/abas por provedor** (ordenado por `pickerOrder`: Claude → ChatGPT/Codex → OpenCode → Gemini → Custom):
   - Ícone oficial de cada provedor (`ProviderLogo`), nome da conta ativa (quando há múltiplas contas ou pool) e estado vivo (`Conectado`, `Verificando`, `Baixando...`, `Desconectado`, `Erro`).
   - Botão inline de ação contextual quando o provedor não está pronto (**Baixar CLI**, **Conectar**, **Tentar novamente**), sem obrigar o usuário a sair do fluxo.
2. **Organização inteligente dos modelos**:
   - **Busca instantânea** por nome do modelo ou sub-serviço/lab.
   - **Agrupamento hierárquico**: no OpenCode (e OpenRouter/ACP), separa `serviço/modelo` (ex.: `OpenCode · Grátis`, `OpenCode Go`, `Anthropic`, `OpenAI`) e ordena modelos **Grátis** primeiro (`isFreeOpencodeModel` + `OPENCODE_RECOMMENDED_FREE_MODELS`).
   - **Variantes de esforço (Reasoning Effort) colapsadas**: modelos como `modelo/low`, `modelo/medium`, `modelo/high`, `modelo/xhigh`, `modelo/max` não poluem a lista — aparecem como **um único item** com um sub-seletor de esforço (`none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max`, `ultra`) e toggle de modo **Fast / Ultrafast** quando suportado (ex.: Codex Fast tier).
3. **Integração direta com Limites de Uso e Backup**:
   - Se a janela de 5h ou semanal do provedor/modelo está esgotada (`spentUsageWindow`), o próprio item no seletor indica visualmente **bloqueado até `HH:MM`** (ou **"Usando créditos"** no Codex quando `creditsAvailable === true`, mantendo o modelo selecionável).
   - Permite configurar uma **cadeia de Backup Models** (até 8 modelos ordenados) por agente com failover automático e retorno automático ao modelo principal após o reset.

#### B. Seletor de Agentes (`AgentPicker.tsx` + `SearchPicker.tsx`)

O OpenBot extrai um primitivo reutilizável (`SearchPicker<T>`) que alimenta o **`AgentPicker`**:
- Popover com campo de busca instantânea + navegação por teclado (`↑/↓/Enter/Esc`).
- Cada linha exibe: **avatar do agente** (`leading`), **nome + papel/subtítulo**, e **contador opcional à direita** (`trailing`, ex.: tarefas ativas ou pendentes daquele agente).
- Usado de forma consistente em toda a UI: filtro de tarefas, atribuição de rotinas, filtro da tela de Estatísticas/Uso, escolha de destinatário/delegação e workflows.

---

### 2.3 Buscas e pesquisas (app, histórico, ferramentas e web/browser)

O OpenBot resolve "busca e pesquisa" em **4 camadas distintas**, cada uma otimizada para custo de tokens e latência:

| Camada | Como funciona no OpenBot | Por que importa para o Vokia |
|---|---|---|
| **1. Busca Global na UI (`GlobalSearch.tsx` + `global-search-sources.ts`)** | Paleta unificada (estilo `⌘K`) com múltiplas fontes: **agentes**, **canais**, **rotinas**, **ações rápidas** (`Novo agente`, `Marketplace`), **abas de configurações** e **arquivos/anexos em conversas** (`searchConversationFiles`, paginado por cursor, limite 50 por página) + busca em mensagens de thread (`thread-message-search-v1`). | No Vokia, uma paleta `⌘K` pode buscar projetos, cards do backlog (`PIX-4`), planos, artefatos, memórias, conversas, arquivos e ações da barra — tudo em um único atalho. |
| **2. Busca sob demanda de ferramentas (`tool_search` / `tool_describe` / `tool_call`)** | `tool-catalog.ts` expõe apenas ~11 ferramentas diretas sempre carregadas e mantém o restante atrás de **3 meta-ferramentas de descoberta**. `tool_search` faz busca local por palavras-chave (até 10 resultados) e já embute o schema completo nos até 3 resultados "fortes" para evitar um turno extra de `tool_describe` (cada turno reenvia o contexto inteiro!). Há inclusive um teste de CI (`check:agent-context`) que falha se o orçamento de bytes do prompt/catálogo crescer. | O `CLAUDE.md` do Vokia já determina manter `tools/list` enxuto via `commands/list` e `commands/call`. Evoluir `commands/list` para um `command_search` com retorno de schema inline para matches fortes reduz ainda mais turnos e tokens. |
| **3. Busca de histórico local (`history_search` / `history_read`)** | Ferramentas deferidas que permitem ao agente consultar mensagens antigas da própria conversa (e `[work steps]` capturados) **sem chamar outro LLM para resumir**. `history_search` devolve até 10 excertos de 400 chars (teto 8 KB); `history_read` pagina por cursor/`messageId` (teto 16 KB) respeitando o último marcador de reset de contexto. | Essencial após compactação ou troca de provedor: o agente recupera decisões e diffs antigos sob demanda sem inflar o prompt inicial. |
| **4. Pesquisa Web e Browser Embutido (`web_search` + CDP)** | **Regra de ouro baseada em benchmark**: provedores com busca inline própria (`OWN_WEB_SEARCH_PROVIDERS`: Claude, Codex, Gemini, Grok) usam sua busca nativa porque criar uma sub-thread externa perde o *prompt cache* (+8% tokens não-cacheados e 2× mais lento: 9,5–12 s vs. 4–6 s). Provedores sem busca nativa (OpenCode sem plugin, Pi, ACP custom) ganham `openbot.web_search`, que dispara uma thread lateral enxuta (sem tools, sem skills, `agents.enabled = false`) e devolve resposta curta com URLs fonte.<br><br>Além disso, possui **browser embutido via CDP** (`browser-tools.ts`) com: `open` já retornando o primeiro snapshot (reduz 3→1 chamada), espera automática de skeletons/DOM settle (`browser-settle.ts`), recusa automática de banners de cookie, detecção de desafios anti-bot com pedido de `request_takeover` humano (sem nunca tentar burlar CAPTCHA), **memórias por domínio (`siteMemories`)** injetadas uma vez por host/turno, e plugins MCP dedicados (ex.: Brave Search). | Para o Vokia (foco em dev), a prioridade de pesquisa é: 1) manter busca nativa nos provedores que já têm; 2) oferecer `vokia.web_search` ou integração MCP (Brave/Exa/Context7 para docs de bibliotecas) aos provedores sem busca; 3) reaproveitar a ideia de browser/preview integrado (já prevista na seção 8.7 da visão de produto) para testar UI e inspecionar docs. |

---

### 2.4 Estatísticas de uso e analytics locais

A camada de analytics de uso (`docs/architecture/analytics.md` + `src/backend/database/agent-usage.ts` + `src/backend/agent/usage-collection.ts`) foi desenhada com regras muito sólidas para evitar distorções comuns:

1. **Armazenamento SQLite desacoplado das conversas**:
   - Tabelas próprias (`migration 15` + índice por data na `migration 16`) **sem foreign key para as mensagens**: limpar o chat ou arquivar uma thread **preserva** todo o histórico de consumo. Só a exclusão definitiva do agente apaga seus registros.
   - Nunca grava conteúdo de prompts, respostas ou credenciais nas tabelas de analytics — apenas contadores numéricos, `provider`, `model`, `agent_id`, `session_id`, `origin` (`chat`, `task`, `routine`, `workflow`) e timestamps.
2. **Contabilização precisa por arquitetura de provedor (sem dupla contagem)**:
   - **Codex**: mantém *checkpoints* cumulativos duráveis por sessão (`:codex:cache-writes`), registrando delta real e separando **`cache_write`** de **`uncached_input`** (erro comum que infla custos estimados).
   - **Claude**: usa `modelUsage` por modelo retornado pela query, com contador próprio por processo de query.
   - **OpenCode e Gemini (Antigravity)**: reportam tokens por prompt; cada prompt recebe seu `promptId` próprio (evitando perda em reentregas), e **`thoughtTokens`** do Gemini entram corretamente como tokens de saída (`output`).
3. **Estimativa de custo USD API-equivalente (transparente)**:
   - Claramente rotulada como **"estimativa equivalente à API"** (não cobrança da assinatura).
   - Tabela de preços versionada por data de vigência (`usage-pricing.ts`), diferenciando faixa normal vs. **long-context** e taxa específica de escrita de cache. Modelos desconhecidos ficam explicitamente sem preço em vez de exibir `$0.00` enganoso.
4. **Consultas e visualização (`AgentUsageReport.tsx` + `usage-series.ts`)**:
   - Filtros de período: **7, 30, 90 dias ou intervalo customizado** (máx. 367 dias), agrupando dias no **fuso horário do usuário** (`inclusive start`, `exclusive next-day`).
   - Uma única consulta SQL agrupa por `(dia, provedor)`, `(dia, modelo)` e `(agente)` — alimentando gráfico de **área empilhada por provedor** + breakdown por agente + tabela diária acessível com valores exatos.

---

### 2.5 Janelas de 5 horas, semanais e controle de limites

Esta é uma das áreas mais avançadas do OpenBot (`packages/team-client/src/plan-limits.ts`, `src/backend/agent/account-usage.ts`, `src/backend/agent/usage-limit-gate.ts` e `docs/architecture/providers.md`).

#### Modelo unificado de cotas (`AccountUsageLimit`)

Todos os provedores são normalizados para a mesma estrutura:
- **`primary`**: janela curta (normalmente **5 horas = 300 min**).
- **`secondary`**: janela longa (normalmente **Semanal = 10.080 min** ou **Mensal = 43.200 min**).
- **`tertiary`** (opcional): terceira janela quando o provedor tem 3 níveis simultâneos (ex.: OpenCode Go tem `rolling 5h`, `weekly 7d` e `monthly 30d`; Cursor tem `Total`, `Cursor Models` e `Other Models`).
- **`groups[]`** (opcional): grupos de modelos com cotas separadas (ex.: Gemini/Antigravity separa *"Gemini Models"* de *"Gemini 4 Argon"*).
- **`creditsAvailable`**: booleano que indica se o plano continua operando em créditos extras mesmo com a janela em 100% (ex.: ChatGPT/Codex).

#### Como o OpenBot classifica janelas e calcula o ritmo (Pace)

Em `plan-limits.ts`:
- **Detecção automática da janela (`planLimitWindowKind`)** com tolerância de ±5%:
  - `~300 min` → **5 horas** (`fiveHour`)
  - `~1.440 min` → **Diária** (`daily`)
  - `~10.080 min` → **Semanal** (`weekly`)
  - `~40.320–43.200 min` → **Mensal** (`monthly`)
  - Outros valores → exibe dinamicamente em `X horas` ou `Y minutos`.
- **Porcentagem restante + severidade (`usageTone`)**:
  - `≥ 30% restante` (≤ 70% usado): `neutral`
  - `10%–29% restante` (71%–90% usado): `warning`
  - `< 10% restante` (> 90% usado): `critical`
  - O anel principal no trilho sempre reflete a **janela mais restritiva** (`bindingWindow` = menor `% restante`), porque qualquer uma delas que zerar trava o agente.
- **Indicador de ritmo (`usagePaceRemainingPercent`)**:
  - Calcula `((resetsAt - now) / windowDuration) * 100`.
  - Permite desenhar uma **marca de ritmo** sobre a barra da janela semanal ou de 5h: se faltam 60% da semana mas só restam 25% da cota, o usuário enxerga imediatamente que está queimando cota acima do ritmo sustentável.

#### Como o OpenBot lê a cota de cada um dos 4 provedores

| Provedor | Fonte técnica usada pelo OpenBot | Janelas obtidas | Observações críticas |
|---|---|---|---|
| **Claude** | Claude Agent SDK (`usage_EXPERIMENTAL` / `rate_limits`) + eventos de stream `rate_limit_event` | `five_hour` (5h) e `seven_day` (semanal, com sub-chave por modelo quando aplicável) | O endpoint de uso da Anthropic compartilha rate limit com o Claude Code e retorna HTTP 429 por 1h+ se consultado com frequência. **Obrigatório usar pacing (`UsageReadGate`).** |
| **Codex (ChatGPT)** | RPC nativo do `codex app-server`: `account/rateLimits/read` + push `account/rateLimits/updated` | `primary` (5h), `secondary` (semanal), `rateLimitsByLimitId` (por modelo), `credits` (`hasCredits`/`unlimited`) e `rateLimitResetCredits` (permite até consumir crédito de reset via `account/rateLimitResetCredit/consume`) | É a integração mais limpa e estável dos 4 provedores. |
| **OpenCode (Go)** | `GET https://opencode.ai/zen/go/v1/usage` com `Authorization: Bearer <key>` (`src/backend/opencode-usage.ts`) | `rolling` (300 min / 5h), `weekly` (10.080 min / 7d) e `monthly` (43.200 min / 30d). A janela secundária escolhe a mais restritiva entre semanal e mensal, e a outra vai para `tertiary`. | Se não há chave salva no app, lê em modo **read-only** (busy timeout 100 ms) na mesma ordem do T3 Code: tabela `credential` do `opencode.db` → `auth.json` (`opencode-go`) → `OPENCODE_API_KEY` → `auth.json` (`opencode`). Modelos gratuitos não têm cota Go. |
| **Gemini (Antigravity)** | `POST cloudcode-pa.googleapis.com/v1internal:loadCodeAssist` → obtém `cloudaicompanionProject` → `POST daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary` (`src/backend/antigravity-usage.ts`) | Grupos de modelos (`groups[]`), cada um com buckets classificados por regex em **5 horas** (`5h / five-hour`) e **Semanal** (`week`). | Usa o `access_token` local do arquivo de token sem tentar dar refresh (só uma sessão CLI ativa renova). Requer `User-Agent` compatível com o cliente desktop. Quando o usuário usa `GEMINI_API_KEY`, não há cota de plano (somente erro de quota/429). |

#### Pacing de leitura (`UsageReadGate`) e Gate de Failover (`UsageLimitGate`)

Dois mecanismos do OpenBot evitam bugs gravíssimos em produção:
1. **`UsageReadGate`** (inspirado no T3 Code, CodexBar e OpenUsage):
   - UI faz polling a cada **5 min**, mas o backend **deduplica chamadas simultâneas** (1 leitura ativa por provedor/conta) e serve cache por **4 min**.
   - Se a leitura falhar (ex.: 429), aplica **backoff exponencial**: `5 min → 10 min → 20 min → 40 min → 60 min`, mantendo o último valor na tela.
   - Se uma janela acabou de cruzar o `resetsAt`, fura o cache de 4 min para atualizar imediatamente (exceto durante backoff de erro).
2. **`UsageLimitGate`** (`src/backend/agent/usage-limit-gate.ts`):
   - Chaveia bloqueios pela tripla **`(provider, accountId, model)`** — nunca trava um agente do Codex porque o Claude bateu o limite!
   - Quando atinge 100%, agenda timer automático para **`resetsAt + 30s`** (ou recheck a cada 15 min se o reset não foi informado) e libera automaticamente a fila quando renova.
   - **Failover inteligente (`!acted`)**: se um turno falhou por limite ou erro de provedor **antes** de escrever texto ou executar tool (`!acted`), devolve a mensagem à fila, coloca o modelo em cooldown de 10 min e reexecuta transparentemente no primeiro **backup model** disponível; grava `usageLimitReturn` para voltar ao modelo original assim que a janela renovar!

---

### 2.6 Etapas, rastreamento e motor anti-abandono de tarefas

O ponto que mais chama atenção para o Vokia — *"as etapas realizadas para a tarefa, para não deixar a tarefa se perder"* — é implementado no OpenBot por **quatro engrenagens integradas**:

#### 1. Plano vivo normalizado por turno (`ConversationPlan` + `plan-updates.ts`)
Independentemente do provedor escolhido, o OpenBot normaliza o checklist interno do agente para um único contrato (`ipc-conversation-plan.ts`):
```ts
interface ConversationPlan {
  explanation: string | null;
  steps: Array<{
    id: string;
    text: string;
    activeText?: string; // gerúndio exibido enquanto executa ("Rodando testes...")
    status: "pending" | "inProgress" | "completed";
  }>; // limite de 100 etapas
}
```
Como cada provedor alimenta esse plano:
- **Claude**: `foldClaudePlanCall` e `foldClaudePlanResult` interceptam chamadas nativas de **`TodoWrite`**, **`TaskCreate`** e **`TaskUpdate`** direto do stream de ferramentas (mapeando IDs temporários para os IDs definitivos retornados no `tool_result`).
- **Codex**: consome diretamente a notificação RPC **`turn/plan/updated`**.
- **OpenCode / Gemini (ACP)**: converte entradas de plano ACP via **`acpPlanSteps`**.
- **UI**: renderiza um card de plano vivo na conversa (`agent-plan-card-v1`) e oculta automaticamente planos anteriores substituídos por turnos mais novos (`replacedConversationPlans`), mantendo a interface limpa.

#### 2. Motor Anti-abandono (`PlanContinuation` em `plan-continuation.ts`)
É o mecanismo que impede o agente de "largar a tarefa pela metade" — problema clássico em agentes autônomos. Ao fim de cada turno, `PlanContinuation.consider()` decide se deve injetar uma continuação automática na fila, com **5 modos**:

1. **`plan` (Etapas abertas no plano)**: se o plano vivo ainda tem etapas `pending` ou `inProgress` (excluindo etapas que já vinham abertas de um plano anterior não relacionado), continua automaticamente listando as etapas pendentes.
   - *Proteção contra loop infinito*: calcula a assinatura `JSON.stringify(openSteps)` — se o turno terminou com exatamente as **mesmas etapas abertas sem progresso**, retorna `STOPPED` e entrega o controle ao usuário.
2. **`nextStep` (Agente anunciou o próximo passo mas parou)**: usa detectores heurísticos **em inglês e português** (`PT_ANNOUNCES_WORK`, `ANNOUNCES_WORK`, `STATES_WORK`, `ASKS_TO_CONTINUE`) para pegar respostas curtas (≤ 400 chars) como:
   - *"Agora vou rodar os testes..."*, *"Deixa eu verificar o arquivo..."*, *"Shall I continue with the remaining steps?"*
   - Respeita exceções explícitas de espera pelo usuário (`waitsForUser`, `PT_NOT_WAITING`: *"se você aprovar"*, *"aguardando sua confirmação"*, perguntas reais terminadas em `?`).
   - Limite de até **2 empurrões `nextStep`** por cadeia.
3. **`finishCheck` (Auditoria de conclusão)**: quando habilitado, faz uma checagem final (até **3 ciclos**) para confirmar se não ficou lacuna (`gap`) entre o pedido e o entregue; se houver lacuna, continua com `finishCheckContinuationPrompt(gap, current, limit)`.
4. **`compaction` (Recuperação pós-compactação)**: se a compactação de contexto interrompeu o turno, retoma automaticamente (1×) para não abandonar o fio.
5. **`providerExit` (Recuperação pós-queda da CLI)**: se o processo do provedor morreu inesperadamente no meio do turno, retoma automaticamente (1×) com contexto preservado.

> Se o usuário mandar uma nova mensagem (`othersQueued`) ou clicar em Parar (`cancel`), a cadeia de continuação é cancelada imediatamente.

#### 3. Rastreamento legível de passos em tempo real (`step-labels.ts`)
Em vez de mostrar apenas *"Executando Bash..."* ou payloads brutos, `stepLabel(item)` traduz cada evento de ferramenta em um rótulo conciso e amigável (com segredos redigidos):
- Ações estruturadas do Codex (`commandActions`: `read`, `listFiles`, `search`) → *"Lendo `providers.rs`"*, *"Listando `src/`"*, *"Buscando `'rateLimit'`"*.
- Comandos shell quebrados por pipe/operadores (`&&`, `||`, `;`), ignorando wrappers (`bash -lc`, `sudo`, `cd`, `export`) → mapeia `rg`/`grep` (*buscando texto*), `git status/diff/log` (*verificando git*), `cat`/`jq`/`sed` (*lendo arquivo*), `curl`/`wget` (*acessando host*), `du`/`df`/`ps` (*medindo disco/processos*).
- Buscas web (`webSearch`, `openbot.web_search`) → *"Pesquisando na web: '...' "*.

#### 4. Captura de `[work steps]` e Handoff de 32 KB (`handoff-tool-steps.ts` + `providers.md`)
Quando uma sessão troca de provedor (ex.: Claude bateu limite → cai para Codex ou Gemini) ou precisa rotacionar por contexto cheio, o OpenBot **não** começa do zero nem envia um dump gigante. Ele monta um **pacote determinístico de handoff de até 32.000 caracteres**:
- **24.000 chars**: mensagens recentes (preservando início e fim de mensagens muito grandes);
- **4.000 chars**: excertos mais antigos;
- **2.000 chars**: **`[work steps]` capturados** (comandos executados + exit code + últimos 600 chars da saída, lista de arquivos alterados, chamadas MCP/tools com status, buscas web feitas e notas de comentário — **sem** o reasoning cru nem outputs gigantes);
- **2.000 chars**: instruções de continuidade e ponteiros para `history_search` / `history_read`.

---

### 2.7 Outros diferenciais relevantes do OpenBot

- **Steering vs. Queue por provedor (`busy-message mode`)**: permite enviar uma correção durante o turno em andamento (`steer`) com bloco `<openbot_steer>` instruindo o modelo a não abandonar o trabalho atual, ou enfileirar (`queue`) para o próximo turno; se o steer chegar tarde e o turno encerrar sem lê-lo (`unreadSteers`), o OpenBot **devolve automaticamente** a mensagem para a fila (`restoreUnsteered`) — nada se perde!
- **Side threads / Tasks paralelas por agente (`AgentTasks`)**: um mesmo agente pode abrir subtarefas paralelas (com limite configurável, padrão 4), ramificar uma conversa a partir de qualquer mensagem (*Branch from here*), e ter sua fila FIFO persistida em SQLite à prova de crash.
- **Revisão de Memória e Aprendizado de Skills em background (`memory-review.ts` + `skill-learning.ts`)**: inspirado no *Hermes Agent*, a cada N turnos (padrão 20) ou 15 min ociosos, roda uma chamada barata sem tools (ex.: Haiku) que revisa o chat + até 80 passos de ferramentas (priorizando falhas e o passo seguinte que funcionou) para atualizar memórias e propor/criar/revisar `SKILL.md` automaticamente — economizando ~3× menos tokens que pedir ao agente principal para salvar memórias durante o turno (20k vs. 63k tokens).
- **Workflows visuais com saídas tipadas e loops controlados**: grafos de nós (trigger → agente/modelo → condição determinística sem LLM `priority = high and count > 2` → aprovação humana → saída GitHub/Slack/notificação) com contratos de resposta tipados (`name: type`).
- **Orçamento de contexto automatizado (`check:agent-context`)**: teste que mede em bytes todas as instruções e schemas injetados nos agentes para impedir inflação silenciosa de tokens.

---

## 3. Levantamento completo de orquestradores de IA para desenvolvimento

O ecossistema de orquestração de agentes de código em 2025–2026 amadureceu em torno de **5 grandes famílias arquiteturais**:
1. **Workspaces Multi-Agente Desktop/IDE** (OpenBot, Vokia, T3 Code, Orca, Superset, Emdash, Conductor, Crystal)
2. **Orquestradores Visuais / Canvas & Kanban** (Maestri, Vibe Kanban, OpenHands Agent Canvas)
3. **Orquestradores Orientados a Backlog / Fábrica Autônoma** (OpenAI Symphony, Gas Town / Gas City, Paperclip)
4. **Enxames & Multiplexers de Terminal (TUI/CLI)** (Claude Code Agent Teams, Claude Squad, OpenCode + `oh-my-opencode`, Agent-Deck, Amux, Overstory)
5. **Agentes Autônomos 24/7 & Utilitários de Telemetria** (Hermes Agent, OpenClaw, CodexBar, OpenUsage)

### 3.1 Matriz comparativa geral

| Orquestrador | Foco / Metáfora | Interface | Multi-provedor (Claude/Codex/OpenCode/Gemini) | Isolamento de código | Rastreamento de etapas / tarefas | Cotas (5h/Semanal) & Analytics | Principal diferencial |
|---|---|---|---|---|---|---|---|
| **Vokia (hoje)** | Time em camadas + Kanban + Planos | Desktop (Tauri + Vue) + MCP/CLI | Parcial (Claude, Codex, OpenCode, `agy`) | Git worktree por feature | Kanban + planos/lotes/dependências + replay | Só Claude (5h/7d); sem analytics completo multi-provedor | Regras visuais entre camadas, triagem em micro-tarefas, artefatos versionados no repo, paridade MCP/CLI |
| **OpenBot** | Coworkers persistentes + Workflows | Desktop (Electron) + Web + Mobile | **Excelente** (18 provedores via SDK/RPC/ACP + Custom) | Diretório / confinamento de processo | Plano vivo (`ConversationPlan`) + **`PlanContinuation`** + `[work steps]` + Side Threads + Workflows | **Excelente** (5h/Semanal/Mensal multi-provedor + Pace + `UsageReadGate` + SQLite analytics) | Infraestrutura de provedores/ACP, leitura real de cotas, motor anti-abandono, browser CDP, memória/skill learning em background |
| **T3 Code** | GUI de engenharia p/ agentes CLI | Desktop + Web + Mobile | **Ótimo** (Codex, Claude, Cursor, OpenCode) | Git worktree + scripts `t3.json` (setup/teardown/portas) | Event sourcing em SQLite, threads, issues | Polling espaçado (5 min) + handoff com teto de tokens | Checkpoints/desfazer por turno, Diff em 4 escopos, *Watch PR/CI*, Queues & Steers, arquitetura pura (decisão vs. efeito) |
| **Orca** | IDE paralela de worktrees + CI loop | Desktop + Mobile + CLI | **Ótimo** (Claude, Codex, Gemini, OpenCode, etc.) | Git worktree por agente/tarefa | Issues GitHub/Linear + automações por gatilho | Métricas básicas por tarefa | Comentário inline na linha do diff que volta ao agente, CI/PR feedback loop, rodar mesma tarefa em 2 agentes e escolher o melhor |
| **Maestri** | Canvas infinito de terminais PTY | Desktop nativo | **Ótimo** (Claude, Codex, Gemini, OpenCode) | Floors/Worktrees + PTY | Nós conectados + sticky notes + status por floor + rotinas | Não focado em cotas | Orquestração espacial visual de agentes em terminais reais, browser portals e Android/devices integrados |
| **Superset** *(ex-Paradev)* | IDE/Terminal para exército paralelo | Desktop nativo | **Ótimo** (Claude, Codex, OpenCode, Aider, Cursor) | Git worktree zero-conflito + gestão de portas | Painel de sessões paralelas + diff unificado | Básico | UX ultra-polida para rodar dezenas de agentes CLI em paralelo com preview/browser embutido |
| **Conductor** | Worktrees locais + Cloud sandboxes | Desktop (Mac) + CLI + Web | Bom (Claude, Codex, etc.) | Branch/worktree + tmux + Cloud sandboxes | Workflows YAML/Markdown determinísticos | Básico | Ponte limpa entre execução local e sandboxes na nuvem com revisões e webhooks |
| **Vibe Kanban** | Kanban visual de agentes *(em sunsetting)* | Web local (`npx`) | **Ótimo** (10+ agentes CLI) | Git worktree + dev server + preview | Board Kanban (Backlog → Done) | Básico | Popularizou Kanban + preview embutido por card; **alerta:** projeto anunciou encerramento/sunsetting |
| **OpenHands** *(ex-OpenDevin)* | Plataforma full-stack autônoma (MIT) | Web / Self-hosted + Agent Canvas + CLI | **Excelente** (model-agnostic + Ollama/vLLM) | Containers Docker / Sandboxes + worktrees | Loops autônomos por issue/PR + event stream | Tracking de tokens/custo por run | Sandboxing forte em containers, triggers GitHub/Slack/cron e maturidade enterprise open-source |
| **Emdash** *(YC W26)* | ADE para times (local, SSH, Cloud) | Desktop + SSH + Cloud | **Ótimo** (Claude, Codex e outros) | Git worktree + ambientes remotos | Linear + GitHub issues sincronizados | Tracking de time | Compartilhamento de MCPs e secrets em equipe + execução híbrida local/SSH/Cloud |
| **OpenAI Symphony** | Spec de backlog → entrega autônoma | Daemon / Control Plane (open spec) | Focado em Codex (extensível) | Workspaces isolados por ticket | Ticket do Linear/tracker → execução → *proof-of-work* | Telemetria de runs | Muda o paradigma de "supervisionar chats" para "gerenciar itens de trabalho com prova de verificação" |
| **Gas Town / Gas City** | "Kubernetes para agentes de código" | CLI Go (`gc`) + Web Dashboard | Multi-CLI (Claude, Codex, etc.) | Worktrees + **Refinery** (fila de merge) | **Beads** (estado durável versionado no Git) | Métricas de throughput | Escala para dezenas de agentes simultâneos com papéis industriais (Mayor, Polecats, Refinery) e persistência em Git |
| **Paperclip** | "Empresa autônoma" (Ops control plane) | Self-hosted Node + React | Multi-agente/runtime | Não gerencia worktrees | Organograma, metas, heartbeats e handoffs | **Budgets financeiros rígidos** por agente | Governança, hierarquia organizacional, permissões e teto de gasto por agente/departamento |
| **Claude Code Agent Teams / Claude Squad** | Enxame nativo / Squad em tmux | Terminal (CLI / `tmux`) | Claude (Agent Teams) / Multi-CLI (Squad) | **File locking** (Teams) / worktree + tmux (Squad) | Task list compartilhada + peer-to-peer messaging | `/usage` nativo (5h/semanal) | Coordenação direta entre agentes pares (Lead + Teammates) com trava de arquivos contra sobrescrita |
| **OpenCode + `oh-my-opencode`** | TUI MIT + orquestra multi-modelo | Terminal (TUI) + ACP server | **Excelente** (qualquer LLM + Ollama + OpenRouter) | Diretório / subagentes | Loop *Sisyphus* / modo *Ultrawork* (Plan-then-Build) | Custo e tokens por sessão | Roteamento de subagentes por especialidade/custo (ex.: Claude planeja, Codex codifica, Gemini revisa) e skills `SKILL.md` |
| **Crystal / Agent-Deck / Amux** | Painéis de execução paralela | Desktop (Crystal) / TUI tmux (Deck/Amux) | Multi-CLI (Claude, Codex, OpenCode, Gemini) | Git worktrees (Crystal) / tmux sessions | Sessões lado a lado + comparação de diffs | Visão consolidada de custo/tokens (Agent-Deck) | Experimentação paralela ("rodar 3 abordagens e ficar com o melhor diff") e supervisão mobile/remota (Amux) |
| **Hermes Agent / OpenClaw** | Autonomia 24/7 + Omnichannel | Server/Docker (Hermes) · Apps de chat (OpenClaw) | Multi-modelo | Sandbox / servidor dedicado | Cron autônomo + auto-criação de skills | Logs/métricas por run | **Hermes**: loop de auto-aprendizado de `SKILL.md` que inspirou o OpenBot. **OpenClaw**: controle via WhatsApp/Telegram/Discord + hub de skills |

---

### 3.2 Análise detalhada por orquestrador

#### 1. **T3 Code** (`pingdotgg/t3code`) — *Referência em arquitetura limpa e ciclo de edição*
- **O que faz melhor que quase todos:** separa estritamente **decisão** de **efeito colateral** (*"Orchestration stays pure, UI stays dumb"*), grava todo estado da orquestração como **event log em SQLite**, oferece **checkpoint por turno com desfazer** (revertendo código + estado da conversa), diff em 4 escopos, vigilância automática de PR/CI (*Watch a pull request*), **Queues vs. Steers** e **handoff de contexto entre provedores** com teto de tokens.
- **O que aproveitar no Vokia:** além do que já está no [plano-de-melhorias.md](plano-de-melhorias.md), o padrão de polling espaçado de cotas e leitura do banco `opencode.db` para uso do OpenCode Go.

#### 2. **Orca** (`stablyai/orca`) — *Referência em revisão de código e loop com CI*
- **O que faz melhor:** fecha o ciclo *código → revisão → CI → merge* dentro do app: comentários na linha do diff viram pedidos diretos ao agente, falhas de CI/reviews no GitHub voltam automaticamente ao agente com limite de tentativas, permite **corrida paralela** (rodar a mesma task em dois agentes/modelos diferentes e escolher o melhor diff) e mantém **paridade total entre UI e CLI**.
- **O que aproveitar no Vokia:** o Vokia já adotou a regra de ouro de paridade MCP/CLI no `CLAUDE.md`; vale absorver o modo "A/B de dois agentes/provedores" no futuro (item 046) e os comentários inline no diff.

#### 3. **Maestri** (`themaestri.app`) — *Referência em orquestração visual com terminais reais*
- **O que faz melhor:** em vez de abstrair tudo em caixas de chat, coloca terminais PTY reais de Claude, Codex, Gemini e OpenCode em um **canvas infinito**, permite ligar agentes com setas, anexar notas adesivas, portais de browser e emuladores Android, além de gerenciar **floors** (worktrees isolados) e rotinas agendadas.
- **O que aproveitar no Vokia:** o Vokia já possui o conceito de **Mapa do time / Orquestração visual**; pode se inspirar na clareza do estado de cada agente/floor e nos portais de browser/preview integrados.

#### 4. **Superset** (ex-Paradev) e **Crystal** — *Referência em UX desktop de worktrees paralelos*
- **O que fazem melhor:** foco obsessivo em eliminar atrito ao rodar vários agentes CLI em paralelo no mesmo repositório — criação instantânea de worktree, alocação automática de portas sem conflito, terminal + preview embutido e comparação limpa de diffs.
- **O que aproveitar no Vokia:** variáveis e alocação de porta por worktree (item 036) + painel de comparação rápida de mudanças por worktree/micro-tarefa.

#### 5. **OpenAI Symphony** e **Gas Town / Gas City** (Beads) — *Referência em durabilidade e escala*
- **Symphony:** formaliza a transição de *chat-centric* para *work-item-centric* — o agente pega um card, trabalha isolado, executa a verificação e entrega um **pacote de evidência (*proof-of-work*)** para aprovação humana. Isso valida 100% a tese central do Vokia (*"Evidência antes de pronto"* + Replay de feature).
- **Gas Town / Beads:** resolve o problema de "o agente se perdeu após crash ou janela cheia" gravando o grafo de trabalho e estado de execução em **Beads** (estruturas leves versionadas no próprio Git) e usando um agente dedicado (**Refinery**) apenas como fila de merge/rebase para evitar caos quando vários agentes terminam ao mesmo tempo.
- **O que aproveitar no Vokia:** reforçar a persistência de etapas/progresso em arquivos versionados no worktree (`.vokia/progress.md` / estado estruturado) e considerar uma etapa/fila dedicada de integração/merge quando múltiplas micro-tarefas fecham em paralelo.

#### 6. **OpenHands** (ex-OpenDevin) e **Emdash** — *Referência em sandboxing e colaboração em time*
- **OpenHands:** padrão-ouro open-source em isolamento via containers/sandboxes reproduzíveis e arquitetura orientada a fluxo de eventos (*Event Stream*).
- **Emdash:** destaca-se por ambientes híbridos (local + SSH + Cloud) e compartilhamento de configurações MCP/secrets entre desenvolvedores do mesmo time.

#### 7. **OpenCode + `oh-my-opencode` (OmO)** — *Referência em roteamento multi-modelo*
- **O que faz melhor:** demonstra na prática o padrão **"Orquestra de Modelos"** (*Model Orchestra*): usar um modelo de raciocínio forte (ex.: Claude Opus/Sonnet ou GPT-6) para planejar, modelos rápidos/econômicos (Codex/OpenCode/Haiku/Flash) para implementar em paralelo, e outro modelo (ex.: Gemini ou modelo distinto) para revisar — reduzindo viés de auto-revisão e cortando custos/consumo de janela de 5h.
- **O que aproveitar no Vokia:** o Vokia já tem a arquitetura de **micro-tarefas com `ModelTier` (`basic/intermediate/advanced`)**; combinar isso com **revisão cross-provider** (ex.: se Claude implementou, Codex ou Gemini revisa) eleva muito a qualidade.

#### 8. **Paperclip** — *Referência em governança e orçamento*
- **O que faz melhor:** introduz **budgets rígidos por agente/projeto** (pausa automática ao atingir teto de gasto ou cota) e ciclos de *heartbeat* para manter agentes sincronizados com metas maiores.
- **O que aproveitar no Vokia:** limites de consumo configuráveis por projeto/agente além dos limites globais do plano (5h / semanal).

#### 9. **Claude Code Agent Teams**, **Hermes Agent** e **CodexBar / OpenUsage**
- **Claude Code Agent Teams:** destaque para **file locking** cooperativo entre agentes que trabalham no mesmo repositório (complementa o aviso de colisão do CodeGraph previsto na visão do Vokia).
- **Hermes Agent:** referência absoluta em **auto-melhoria contínua** (transformar correções recorrentes e sequências de ferramentas bem-sucedidas em arquivos `SKILL.md` reutilizáveis).
- **CodexBar / OpenUsage:** referências especializadas em telemetria de cotas (janelas de 5h e 7 dias) lendo credenciais locais sem duplicar login — inclusive serviram de referência direta para o `UsageReadGate` do OpenBot.

---

### 3.3 Padrões convergentes do mercado (2025–2026)

Ao cruzar os 15+ orquestradores pesquisados, ficam claros **7 consensos técnicos** que se tornaram padrão mínimo da categoria:

1. **ACP (Agent Client Protocol) + RPC nativo em vez de CLI "fire-and-forget"**: rodar `cli exec` sem sessão viva impede steering, perde cache de MCP, dificulta leitura de cotas e quebra o rastreamento de sub-etapas. O mercado converge para **SDK/stream vivo** (Claude), **app-server RPC** (Codex) e **ACP** (OpenCode, Gemini/Antigravity, Cursor, Cline, Copilot, Mistral).
2. **Git Worktrees com lifecycle completo**: criação + `setup` + alocação de portas/variáveis + `teardown` + fila/verificação de merge.
3. **Plano vivo + Motor Anti-abandono**: extrair o plano estruturado do agente e reagir automaticamente quando ele para antes de concluir etapas ou anuncia o próximo passo sem executar.
4. **Handoff estruturado entre provedores**: trocar de modelo/provedor sem perder o fio da tarefa usando um pacote enxuto (mensagens recentes + `[work steps]` + ferramentas de busca sob demanda no histórico).
5. **Visibilidade real de cotas em múltiplas janelas (5h + semanal + mensal + pace) + Failover com retorno automático**: o desenvolvedor não pode ser surpreendido por um bloqueio no meio de um lote.
6. **Economia agressiva de contexto**: `tool_search` sob demanda, remoção de prompts/skills redundantes das CLIs hospedadas e uso de modelos leves em micro-tarefas e tarefas de background.
7. **Paridade UI ↔ MCP ↔ CLI**: tudo que o humano faz na interface, o agente e a automação também conseguem acionar com segurança (princípio que o Vokia já cumpre exemplarmente no `CLAUDE.md`).

---

## 4. Plano técnico de adoção no Vokia

Todas as propostas abaixo respeitam as diretrizes de [`CLAUDE.md`](../CLAUDE.md) (lógica em `commands/*.rs` via `_exec`, exposição unificada em `dispatcher.rs` para MCP + `vokia-cli`, frontend Vue 3 + Pinia com `tokens.css` sem cores fixas).

### 4.1 Arquitetura para os 4 provedores (Claude, Codex, OpenCode, Gemini)

#### Matriz alvo dos 4 provedores iniciais no Vokia

| Provedor (`id`) | Nome exibido | Protocolo alvo no Rust | Sessão & Steering | Ferramentas Vokia | Leitura de Cotas (5h / Semanal) |
|---|---|---|---|---|---|
| `claude` | **Claude Code** | `ClaudeLive` (já existe; evoluir com troca de modelo/effort sem restart + supressão de tools host-only) | Sessão viva + steer nativo | MCP nativo (`--mcp-config`) | `rate_limit_event` do stream + sonda espaçada (`UsageReadGate`) |
| `codex` | **Codex (ChatGPT)** | **Evoluir de `CodexJsonl` (`codex exec`) para `CodexAppServer` (JSON-RPC persistente)** com pool ocioso (5 min/thread, 30 min/app-server) | Sessão viva + steer nativo + resume | MCP nativo (cache 30 min) | RPC `account/rateLimits/read` + push `account/rateLimits/updated` + `creditsAvailable` |
| `opencode` | **OpenCode** | **Evoluir de `OpenCodeJson` (`opencode run`) para `OpenCodeAcp` (`opencode acp`)** + relay HTTP local para MCP (`127.0.0.1` + bearer) | Sessão ACP + `_session/steering` (v2.3+) + recovery de subagentes | MCP nativo via relay HTTP (destrava modo `Liberado` e remove fragilidade do `VokiaCli` em modo Leitura) | `GET https://opencode.ai/zen/go/v1/usage` (5h, semanal, mensal) + badge **Grátis** para tier gratuito sem conta |
| `gemini` *(alias `antigravity`)* | **Gemini** | **Suporte dual ou unificado via ACP:**<br>1) **Antigravity CLI** (`agy acp` / `antigravity acp`) para planos **Google AI Pro/Ultra**;<br>2) Opcional: **Gemini CLI** (`gemini`) para quem usa Google Code Assist / `GEMINI_API_KEY` direto | Sessão ACP + steer por prompt (v1.5+) + rotação por handoff quando contexto encher | MCP nativo via ACP (fallback `VokiaCli`) | Code Assist (`loadCodeAssist` + `retrieveUserQuotaSummary`) com grupos de modelos (5h + semanal) |

> **Decisão recomendada sobre ACP:** implementar **um único cliente ACP em Rust** (`desktop/src-tauri/src/orchestrator/adapters/acp/`) com "flavors" pequenos por provedor (`opencode`, `gemini/antigravity`). Isso resolve **OpenCode e Gemini de uma só vez** e já deixa o Vokia pronto para adicionar Cursor, Cline, Copilot, Mistral ou agentes ACP customizados no futuro quase sem código novo.

#### Melhorias de conexão e autenticação
- **Verificação real de status (`provider_status_exec`)**: distinguir claramente `Não instalado` → `Instalado, sem login` → `Conectado (versão X)` → `Erro de autenticação/rede`.
- **Fluxo de login assistido por provedor** (mantendo o princípio do Vokia de **não guardar senhas** — a CLI continua dona da credencial):
  - Botão **"Conectar"** na tela de Provedores / Onboarding dispara o fluxo nativo da CLI (`claude auth login`, `codex login --device-auth`, `agy acp --login`, ou campo de chave opcional para OpenCode Go / `GEMINI_API_KEY`), exibindo link/código no modal e validando a conexão ao concluir.
- **Variáveis de ambiente opcionais por provedor** (sem expor valores nos logs): permitir configurar `BASE_URL` / gateways compatíveis quando o usuário quiser rotear por proxy local ou corporativo.

---

### 4.2 Seletor de modelos e seletor de agentes

#### 1. Evolução do `ActiveLlmSwitcher.vue` → Seletor estilo OpenBot
Aproveitando a base já construída em `desktop/src/features/providers/ActiveLlmSwitcher.vue`:
- **Coluna/trilho lateral de provedores** à esquerda do modal com os 4 provedores (Claude, Codex, OpenCode, Gemini), cada um com **ícone próprio**, bolinha de estado (`Conectado`, `Ausente`, `Limite atingido`) e mini-indicador de cota.
- **Painel de modelos à direita**:
  - Campo de busca instantânea (`Buscar modelos...`).
  - Se o provedor selecionado não estiver instalado ou autenticado, mostrar estado acionável inline (**Instalar / Conectar / Verificar CLI**) em vez de apenas itens desabilitados.
  - Agrupamento por categoria/serviço (no OpenCode: **Grátis** no topo, seguido de **OpenCode Go** e provedores configurados).
  - **Sub-seletor de Esforço (Reasoning Effort)** (`Básico / Médio / Alto / Máximo`) integrado ao modelo selecionado, evitando duplicar linhas (`modelo-low`, `modelo-medium`, `modelo-high`).
  - **Indicador de cota no item**: se a janela de 5h ou semanal daquele provedor/modelo está em 100%, mostrar badge *"Limite até 18:30"* (ou *"Usando créditos"* no Codex) e destacar o backup ativo.
- **Cadeia de Backup Models por agente e global**: evoluir `fallback.rs` para suportar retorno automático (`usageLimitReturn`) quando a janela do modelo principal renovar e failover transparente quando o erro ocorrer antes de qualquer efeito (`!acted`).

#### 2. Novo componente reutilizável `AgentPicker.vue`
Criar um popover pesquisável (`desktop/src/features/agents/AgentPicker.vue`, usando tokens de `tokens.css`, fechamento por `Escape`/clique fora e gestão de foco):
- Lista todos os agentes do projeto/workspace com **avatar**, **nome**, **papel (role)**, **chip do provedor/modelo atual** (ex.: `Claude · Sonnet`, `Codex · GPT-6`) e **contador de fila/status** à direita.
- **Onde usar no Vokia:**
  - **Barra de ação**: escolher rapidamente quem recebe um pedido direto (além de `@agente`).
  - **Backlog / Modal do card**: trocar o dono de uma micro-tarefa ou escolher quem faz a triagem/revisão.
  - **Estatísticas**: filtrar métricas de tokens, tempo e aprovações por agente específico ou "Todos".
  - **Conversas**: salto rápido entre agentes.

---

### 4.3 Motor de cotas multi-provedor (5h / Semanal / Mensal) e estatísticas

#### A. Backend unificado de cotas (`desktop/src-tauri/src/usage/`)
Substituir o acoplamento exclusivo ao `claude` por um subsistema multi-provedor:
1. **Contrato unificado (`ProviderUsageSnapshot`)**:
   ```rust
   pub struct UsageWindow {
       pub kind: WindowKind,          // FiveHour | Daily | Weekly | Monthly | Custom(u32)
       pub duration_mins: Option<u32>,
       pub used_percent: f64,         // 0.0..=100.0
       pub resets_at: Option<u64>,    // unix seconds
       pub rejected: bool,
   }

   pub struct ProviderUsageGroup {
       pub name: String,
       pub models: Vec<String>,
       pub primary: Option<UsageWindow>,
       pub secondary: Option<UsageWindow>,
   }

   pub struct ProviderPlanUsage {
       pub provider_id: String,       // "claude" | "codex" | "opencode" | "gemini"
       pub primary: Option<UsageWindow>,
       pub secondary: Option<UsageWindow>,
       pub tertiary: Option<UsageWindow>,
       pub groups: Vec<ProviderUsageGroup>,
       pub credits_available: Option<bool>,
       pub updated_at_ms: u64,
       pub read_failed: bool,
   }
   ```
2. **`UsageReadGate` em Rust**:
   - Single-flight (1 leitura por provedor por vez), cache válido por **4 minutos**, polling do frontend a cada **5 minutos**.
   - Backoff exponencial em erro (`5 → 10 → 20 → 40 → 60 min`) mantendo a última leitura válida visível.
   - Invalidação imediata quando `now >= resets_at` ou quando credencial/configuração do provedor muda.
3. **`UsageLimitGate` aprimorado**:
   - Controla holds por `(provider_id, model)` (não globalmente).
   - Timer automático para `resets_at + 30s` que libera a fila e devolve agentes que estavam em modelo de backup (`usageLimitReturn`).
4. **Cálculo de Ritmo (Pace)**:
   - Para cada janela (especialmente a **semanal** e a de **5 horas**), exibir além da barra de `% usada` um marcador vertical indicando **onde o tempo da janela está agora** (ex.: *"42% da semana restante · 65% da cota restante — folgado"* vs. *"70% da semana restante · 25% da cota restante — ritmo alto"*).

#### B. Frontend de Cotas (`usagePlans.ts`, `UsageAvatar.vue`, `UsagePanel.vue`)
- Remover o `PLANS = [{ id: 'cc' }]` fixo: renderizar dinamicamente todos os provedores conectados que reportam uso (**Claude, Codex, OpenCode Go, Gemini**).
- No **anéis do avatar no trilho**: mostrar o estado agregado (pior janela ativa) ou anéis duplos/indicador por provedor, e no **`UsagePanel.vue`** listar uma seção por provedor com:
  - Janela de **5 h**, **Semanal (7 d)** e **Mensal (30 d)** quando existir;
  - Sub-linhas por **grupo de modelos** no Gemini (`Gemini Models`, `Gemini 4 Argon`, etc.);
  - Contagem regressiva até o reset (`renova em 2 h 14` / `renova em 3 dias 4 h`);
  - Indicador visual de **ritmo (pace)**;
  - Badge *"Usando créditos"* quando aplicável (Codex).

#### C. Estatísticas de Uso em SQLite (`db/usage_analytics.rs`)
- Tabela `usage_records` independente da tabela de mensagens (limpar conversa nunca apaga métricas):
  - Colunas: `id`, `occurred_at`, `project_id`, `card_id`, `agent_id`, `provider_id`, `model`, `origin` (`card`, `quick`, `triage`, `review`, `background`), `input_tokens`, `cached_input_tokens`, `cache_write_tokens`, `output_tokens`, `thought_tokens`, `cost_usd_estimated`, `price_basis`, `is_fallback`, `duration_ms`.
- Comandos expostos no `dispatcher.rs` (disponíveis na UI, MCP e `vokia-cli`):
  - `get_plan_usage` (janelas 5h/semanal/mensal atuais de todos os provedores);
  - `get_usage_analytics` (filtros: `days: 7 | 14 | 30 | 90` ou `from/to`, `timezone`, `projectId?`, `agentId?`, `providerId?`) retornando séries diárias por provedor, totais por agente, totais por modelo e economia com micro-tarefas/cache.

---

### 4.4 Rastreamento de etapas e anti-abandono por turno

Aqui está o maior ganho prático para **"não deixar a tarefa se perder"** no Vokia, complementando os planos/lotes que o Vokia já tem:

1. **Plano vivo dentro do turno (`TurnPlan` / `LiveSteps`)**:
   - Capturar automaticamente nos adaptadores:
     - `claude`: interceptar `TodoWrite`, `TaskCreate`, `TaskUpdate` no stream JSON;
     - `codex`: consumir `turn/plan/updated` do `codex app-server`;
     - `opencode` e `gemini`: consumir eventos `plan` do ACP.
   - Emitir novo evento `VokiaEvent::TurnPlanUpdated { project_id, agent, card_id, steps }` e mostrar um **card compacto de etapas ao vivo** na Conversa e no painel *"Acontecendo agora"* da Barra de Ação (com status `pendente / em andamento / concluído` + texto ativo).
2. **Motor Anti-abandono (`orchestrator/continuation.rs`)**:
   - Integrar ao final de cada turno (antes de devolver controle ao humano ou marcar etapa como concluída):
     - **Regra 1 — Etapas abertas no plano do turno**: se ainda há steps `pending`/`inProgress` e houve progresso em relação ao turno anterior (assinatura mudou), dispara continuação automática (teto configurável, ex.: até 3–5 continuações).
     - **Regra 2 — Anúncio sem execução (PT-BR + EN)**: se a resposta é curta e termina anunciando trabalho sem rodar ferramenta (*"Agora vou rodar os testes..."*, *"Vou editar o arquivo..."*, *"Quer que eu continue?"*), e **não** é uma pergunta real para o usuário (`ask_human` / `waitsForUser`), empurra automaticamente (até 2×) para o agente executar em vez de parar.
     - **Regra 3 — Recuperação pós-crash/compactação**: se o processo da CLI cair ou sofrer compactação incompleta no meio de uma micro-tarefa, retoma automaticamente 1× preservando os `[work steps]` já executados.
     - **Integração com `loop_guard.rs` e limite de uso**: toda continuação respeita o teto de tentativas, pausa se o limite de uso (90%) disparar, e é cancelada instantaneamente se o usuário enviar uma mensagem ou pausar o time.
3. **Rótulos humanos por passo (`orchestrator/step_labels.rs`)**:
   - Traduzir eventos de ferramentas e comandos de terminal para frases legíveis em PT-BR (*"Lendo `providers.rs`"*, *"Buscando `'rate_limit'` no código"*, *"Verificando status do git"*, *"Rodando `cargo test`"*, *"Pesquisando na web..."*) para alimentar o status curto da Barra de Ação, do Mapa do Time e das Conversas.
4. **Captura compacta de `[work steps]` para Handoff (integra com o item [042](backlog/042.md))**:
   - Durante cada turno, registrar um resumo compacto (teto 4 KB/turno) de: comandos rodados + exit code + cauda do output, arquivos alterados, ferramentas MCP chamadas e buscas feitas.
   - Ao trocar de provedor (manualmente ou por failover de limite), injetar pacote de handoff limitado (ex.: 24–32 KB) contendo mensagens recentes + `[work steps]` + etapas abertas do plano.

---

### 4.5 Busca global, descoberta de ferramentas e pesquisa

1. **Paleta de Busca Global (`⌘K` / `Ctrl+K`)**:
   - Busca unificada em: **Projetos**, **Cards/Features do Backlog**, **Agentes** (salto direto para conversa), **Planos**, **Artefatos**, **Memórias**, **Mensagens de conversas** (FTS5 no SQLite) e **Ações/Configurações**.
2. **Descoberta sob demanda de comandos no MCP (`command_search`)**:
   - Evoluir o catálogo em `commands/dispatcher.rs` para incluir um comando de busca por palavra-chave que devolve os schemas completos dos top 3 resultados fortes — mantendo `tools/list` mínimo e evitando um turno extra de leitura.
3. **Busca de histórico local pela IA (`history_search` e `history_read`)**:
   - Expor como comandos no `dispatcher.rs` (acessíveis via MCP e `vokia-cli`) para que agentes consultem mensagens e `[work steps]` antigos da mesma feature/card sem precisar injetar todo o histórico no prompt.
4. **Estratégia de Pesquisa Web para agentes**:
   - **Claude, Codex e Gemini**: usar busca web nativa da própria CLI (preserva *prompt cache* e menor latência).
   - **OpenCode (e provedores sem web search nativa)**: oferecer uma ferramenta `web_search` opcional via MCP do Vokia (seja via conector MCP como Brave/Exa/Context7 ou delegando para um agente/provedor de pesquisa configurado no time), respeitando a arquitetura de camadas do Vokia (camada **Pesquisa**).
5. **Revisão assíncrona de memórias e skills (futuro, inspirado em OpenBot/Hermes)**:
   - Rodar em background (usando modelo `Basic`, ex.: Haiku / Flash / GPT mini) ao encerrar uma feature ou após N turnos, extraindo lições e sugerindo novas memórias/skills na barra sem interromper nem gastar contexto do turno principal.

---

### 4.6 Priorização sugerida por ondas

Alinhada ao [plano-de-melhorias.md](plano-de-melhorias.md) (complementando o ciclo de entrega com a camada multi-provedor, cotas e confiabilidade de etapas):

| Onda | Entregas (documentos no backlog) | Impacto direto |
|---|---|---|
| **Onda A · Cotas multi-provedor, estatísticas e seletores** | 1) [083](backlog/083.md) Motor unificado de cotas multi-provedor (`ProviderPlanUsage`, `UsageReadGate`, 5 h / Semanal / Mensal + Pace + retorno automático);<br>2) [084](backlog/084.md) Leitores de cota para **Claude**, **Codex**, **OpenCode Go** e **Gemini**;<br>3) [085](backlog/085.md) Seletor de provedor, modelo e esforço estilo OpenBot (`ActiveLlmSwitcher.vue`);<br>4) [086](backlog/086.md) Seletor pesquisável de agentes reutilizável (`AgentPicker.vue`);<br>5) [087](backlog/087.md) Banco de estatísticas de uso desacoplado (`usage_records`) e consultas por período. | Resolve a cegueira de cotas fora do Claude, entrega a experiência de seleção estilo OpenBot para os 4 provedores e agentes, e viabiliza a tela de Estatísticas real. |
| **Onda B · Rastreamento de etapas e anti-abandono** | 1) [088](backlog/088.md) Plano vivo de etapas por turno multi-provedor (`TurnPlan`);<br>2) [089](backlog/089.md) Motor anti-abandono e continuação automática de turnos (`PlanContinuation` com detecção PT-BR/EN e recuperação pós-crash);<br>3) [090](backlog/090.md) Rótulos humanos de passos em tempo real (`step_labels.rs`);<br>4) [091](backlog/091.md) Captura compacta de etapas de trabalho (`[work steps]`) para handoff de 32 KB (conecta com [042](backlog/042.md)). | Impede que tarefas se percam no meio de um turno ou numa troca de provedor e dá visibilidade clara do que cada agente está fazendo. |
| **Onda C · Protocolos modernos e conexão dos 4 provedores** | 1) [092](backlog/092.md) Adaptador Codex persistente via `codex app-server` (RPC, pool ocioso, cache MCP, resume, steering e cotas);<br>2) [093](backlog/093.md) Cliente **ACP em Rust** para OpenCode (`opencode acp`) e Gemini (`antigravity acp` / opção `gemini`) com relay HTTP local para MCP;<br>3) [094](backlog/094.md) Fluxo assistido de conexão, verificação e variáveis/gateways de provedores. | Tira os 4 provedores do modo "exec efêmero" para o padrão de primeira classe (sessão viva, MCP nativo, steering, login claro e cotas reais). |
| **Onda D · Buscas, descoberta e pesquisa** | 1) [095](backlog/095.md) Paleta de Busca Global (`⌘K` / `Ctrl+K`) com SQLite FTS5;<br>2) [096](backlog/096.md) Busca sob demanda de comandos (`command_search`) e histórico (`history_search` / `history_read`) no MCP/`vokia-cli`;<br>3) [097](backlog/097.md) Pesquisa web unificada para provedores sem busca nativa;<br>4) [098](backlog/098.md) Revisão assíncrona de memórias e sugestão de skills em background (modelo `Basic`). | Reduz consumo de contexto, melhora navegação em múltiplos projetos e fecha o ciclo de aprendizado contínuo do time. |

---

## 5. Fontes e referências

- **OpenBot (código-fonte e documentação arquitetural inspecionados diretamente):**
  - Repositório: [`nightly-labs/openbot`](https://github.com/nightly-labs/openbot) (`openbot.run`)
  - Documentação: `docs/architecture/providers.md`, `docs/architecture/analytics.md`, `docs/architecture/agents.md`, `docs/architecture/integrations.md`, `docs/architecture/browser.md`, `docs/architecture/events.md`, `docs/architecture/side-panel.md`
  - Módulos analisados: `packages/contracts/src/agent-providers.ts`, `packages/contracts/src/ipc-conversation-plan.ts`, `packages/team-client/src/plan-limits.ts`, `packages/ui/src/features/account/account-usage-view.ts`, `packages/ui/src/components/ProviderModelPicker.tsx`, `packages/ui/src/components/provider-model-options.ts`, `packages/ui/src/features/agents/AgentPicker.tsx`, `src/backend/opencode-usage.ts`, `src/backend/antigravity-usage.ts`, `src/backend/claude-client.ts`, `src/backend/agent/account-usage.ts`, `src/backend/agent/usage-limit-gate.ts`, `src/backend/agent/plan-updates.ts`, `src/backend/agent/plan-continuation.ts`, `src/backend/agent/handoff-tool-steps.ts`, `src/backend/agent/step-labels.ts`, `src/renderer/src/global-search-sources.ts`
- **Orquestradores e ferramentas comparadas:**
  - **T3 Code**: [`pingdotgg/t3code`](https://github.com/pingdotgg/t3code)
  - **Orca**: [`stablyai/orca`](https://github.com/stablyai/orca)
  - **Maestri**: [`themaestri.app`](https://themaestri.app)
  - **Superset (ex-Paradev)**: [`paradev.com`](https://paradev.com)
  - **Conductor**: [`conductor.build`](https://conductor.build) / Conductor OSS
  - **Vibe Kanban**: repositório público (aviso de sunsetting em 2026)
  - **OpenHands**: [`openhands.dev`](https://openhands.dev)
  - **Emdash**: [`emdash.com`](https://emdash.com)
  - **OpenAI Symphony**: especificação e implementação de referência OpenAI
  - **Gas Town / Gas City (Beads)**: projeto de orquestração em escala (Steve Yegge)
  - **Paperclip**: [`paperclip.ing`](https://paperclip.ing)
  - **OpenCode & `oh-my-opencode`**: [`opencode.ai`](https://opencode.ai)
  - **Hermes Agent** (Nous Research) e **OpenClaw**: [`hermes-agent.org`](https://hermes-agent.org) / [`openclaw.ai`](https://openclaw.ai)
  - **Claude Code Agent Teams**, **Claude Squad**, **Crystal**, **Agent-Deck**, **Amux**, **CodexBar** e **OpenUsage**
- **Código e documentação interna do Vokia analisados:**
  - [`CLAUDE.md`](../CLAUDE.md), [`docs/vokia-visao-de-produto.md`](vokia-visao-de-produto.md), [`docs/plano-de-melhorias.md`](plano-de-melhorias.md)
  - `desktop/src-tauri/src/providers.rs`, `desktop/src-tauri/src/orchestrator/adapters/{mod,codex,opencode,antigravity}.rs`, `desktop/src-tauri/src/orchestrator/{plan_stages,fallback}.rs`, `desktop/src-tauri/src/claude/stream.rs`
  - `desktop/src/features/providers/ActiveLlmSwitcher.vue`, `desktop/src/features/usage/{usagePlans,useUsage,UsagePanel}.ts/.vue`
