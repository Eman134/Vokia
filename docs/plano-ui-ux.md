# Plano de UI/UX · Tasks, card e Conversas

Levantamento de 2026-11-24, feito a partir de capturas do app em uso (quadro com 46 itens no Backlog e 60 no Done, modal do `item-88` e o canal `#geral` durante uma validação do Axiom) e da leitura do código em `desktop/src/`. Complementa o [plano de melhorias](plano-de-melhorias.md), que trata do processo e da orquestração. Este documento trata só de **como o usuário vê e opera** o que já existe.

## 1. Resumo

O Vokia mostra tudo com o mesmo peso. Card concluído e card que pede decisão ocupam o mesmo espaço. Comando aprovado automaticamente aparece com o mesmo destaque da resposta do agente. Ação destrutiva fica ao lado de delegar e de mover. O resultado é muita rolagem, muito texto técnico na frente e pouca indicação do que precisa do usuário agora.

As melhorias seguem três ideias:

1. **Densidade proporcional à atenção.** O que pede ação fica expandido e o resto fica compacto, agrupado ou recolhido.
2. **Uma ação principal por contexto.** As outras ficam num menu ou num link secundário.
3. **Detalhe técnico sob demanda.** Comandos, caminhos e IDs aparecem ao expandir, não no fluxo principal.

## 2. Diagnóstico

### 2.1 Tela de Tasks

| Problema | Causa no código | Efeito |
|---|---|---|
| A página inteira rola, e a altura é a da coluna mais longa | `TasksPage.vue`: `PAGE_CLASSES` tem `overflow-y-auto`; as colunas (`COLUMN_CLASSES`) não têm altura nem rolagem própria | Com 60 cards no Done, quem trabalha no Ready ou no Review rola junto uma tela de ~8000px. O cabeçalho, o "Novo pedido" e os títulos das colunas somem |
| Done com cards cheios | `TaskCard.vue` usa o mesmo layout em todas as colunas; no Done a barra de progresso é sempre 100% (`DONE_SEGMENTS`) | Espaço gasto com um arquivo que ninguém opera |
| 46 botões "Triar" iguais no Backlog | `BacklogPlanActions` aparece sempre que `cardActionFor` devolve ação | Ruído visual repetido; triar vários exige abrir um por um |
| Texto repetido no card | `planLine` (`useItemPlan.lineFor`) repete o título com "· proposta aceita" | No `item-88` o título aparece duas vezes seguidas |
| Post-it da proposta fora de contexto | O bloco `proposal` aparece em qualquer coluna | No In review, o post-it ocupa o lugar do andamento real (etapas, testes, diff) |
| Anexos como chips "image.png" | `AttachmentPreviewList compact` mostra um chip por anexo | Nome sem informação; dois chips para dizer "tem 2 imagens" |
| Filtro solitário | O único filtro é `UiSegmented` de feature, que hoje só tem "Todas" | Ocupa uma linha sem servir; busca, agente e tipo não existem (`item-26` no backlog) |
| ID longo no card | Cards criados por agente recebem o slug do título como ID (ex.: `ids-curtos-para-cards-criados-por`) | Quebra o alinhamento do título. Já está em andamento no card "IDs curtos" |

### 2.2 Modal do card (`TaskItemModal.vue`)

| Problema | Onde | Efeito |
|---|---|---|
| Rodapé com três linhas e onze botões de pesos misturados | `footer`: "Mover para" (chips), "Delegar a" (um chip por agente, incluindo Triage), Remover e Arquivar em vermelho, "Abrir thread", "Abrir conversas" e "Iniciar" | Não há hierarquia. "Remover" fica a um clique de "Triage" e longe de qualquer confirmação visual de perigo |
| Markdown cru no desdobramento | `PlanBreakdown.vue` mostra as tarefas da etapa com crases literais (`` `resolveRelatedItem(...)` ``) | Leitura difícil; o código não se destaca do texto |
| Selo "proposta" repetido três vezes | Uma oval amarela por micro-tarefa | Destaque alto para uma informação que é igual em todas e já está no selo do cabeçalho |
| Mesma informação em três lugares | "3 micro-tarefas · 0 feitas" (desdobramento), "3 micro-tarefas · ~1 dia" (caixa Plano) e "Ligado a · plano · plano-item-88" | A coluna lateral repete a principal |
| Nome e ID misturados | "Responsável: Axiom" e "Aberto por: axiom" | Parece dois agentes diferentes |
| Texto gerado com erro | `src-tauri/src/db/backlog.rs:1951`: "criou o card para o time {}" com o time chamado "Time" vira "para o time Time" | Erro de texto visível |
| Dois idiomas para o mesmo estado | A coluna se chama "Ready" e o selo diz "Pronto para o time"; os botões dizem "Backlog" e "Done" | O usuário traduz de cabeça o tempo todo |
| Modal centralizado esconde o quadro | `OVERLAY_CLASSES` com fundo sobre a tela | Para comparar ou passar para o próximo card é preciso fechar e abrir de novo |

### 2.3 Tela de Conversas

| Problema | Causa no código | Efeito |
|---|---|---|
| Cada comando aprovado automaticamente vira uma linha da conversa | `MessageEntry.vue` mostra um `permissionResultRow` ("Modo bypass", `permissionSummary.ts`) por permissão, fora do `ActivityRun` | No print, 7 linhas seguidas de PowerShell com ~300 caracteres cada. A resposta do agente fica escondida entre elas |
| Delegação em bloco centralizado de duas linhas | `MessageEntry.vue:138`: "Ember → Axiom" e o resumo do pedido embaixo | Cada ida e volta custa ~60px e quebra o eixo de leitura à esquerda |
| Linha de texto sem limite de largura | As mensagens ocupam a largura toda (~1900px num monitor largo) | Linhas de 250 caracteres ou mais; o olho perde a próxima linha |
| "Usou 1 integração ▸" em toda resposta do Ember | `ActivityRun` mostra a frase mesmo quando a única ferramenta é a delegação, que já aparece como bloco | Ruído repetido |
| Threads sem estado | A lista lateral mostra 19 threads com o mesmo ícone, sem agente, atividade ou card | Não dá para saber qual está viva, qual terminou e a que card pertence |
| Agentes livres ocupam o mesmo espaço que os ocupados | `AgentList.vue` lista os 8 agentes em linhas iguais | O único agente trabalhando (Axiom) não se destaca |
| Cabeçalho alto em toda tela | `UiPageHeader` com título grande e "Vokia" manuscrito | ~100px fixos, que fazem falta numa ferramenta aberta o dia todo |

### 2.4 Tela de Artefatos (`ArtefatosPage.vue`)

| Problema | Onde | Efeito |
|---|---|---|
| Tipo dito três vezes em cada card | Aba desenhada "plano", caminho "Artefatos › Plano · item-15" e aba "Planos 45" | Linha inteira de ruído em todos os 48 cards |
| Markdown cru no resumo | `PlanCard.vue` mostra `proposal.summary` como texto: "## Causa raiz", "# Pesquisa:", crases | O resumo lê como o início de um arquivo, não como frase |
| Selo "pronto" para plano concluído | `PlanCard.vue` usa `STATE_CHIP[state]`, o estado do plano, e ignora a coluna do card | Plano de card já no Done aparece como "pronto" |
| "0 etapas" e alturas desiguais | `stageLabel` aparece sempre; a barra só existe quando há etapas | Linha inútil e barras desalinhadas na grade |
| Sem ordem nem agrupamento | `plans` segue a ordem do backlog; concluídos e propostas misturados | O que pede ação fica perdido entre dezenas de planos |
| Sem busca | Só as abas de tipo | Achar o item-49 entre 48 artefatos exige rolar a grade |
| Abas no singular e total repetido | `filterOptions` usa o rótulo do tipo ("Documento 2", "Decisão 1"); `countLabel` repete o total | Concordância errada e informação duplicada |
| Plano sem caminho de volta ao card | `PlanArtifact.vue` não tem link para o card | Para ver o card do plano é preciso ir a Tasks e procurar |

## 3. Princípios

- **O que precisa de você vem primeiro.** Card com pergunta, proposta para aprovar, permissão pendente e erro ganham destaque. O resto é contexto.
- **Densidade por coluna, não por tela.** Backlog e Done são listas para consultar. Ready, In progress e In review são trabalho.
- **Hierarquia de ação.** Por contexto há um botão primário, ações frequentes visíveis como secundárias, as raras num menu `⋯` e as destrutivas sempre no menu, com confirmação.
- **Idioma único na interface.** Português em tudo que o usuário lê, inclusive o nome das colunas. Os valores internos (`'Ready'`, `'Done'`) continuam iguais no código e na API. Só o rótulo muda, via `tasksPageCopy.ts` e `backlogLabels.ts`.
- **Seguir o design system.** Tokens de `styles/tokens.css`, `cursor-pointer` e `disabled:cursor-not-allowed`, e popover e modal com Escape, clique fora e foco devolvido, como manda o `CLAUDE.md`.
- **Validar no navegador.** Mudança visual não ganha teste de componente. Comportamento real (rolagem por coluna, recolher o Done, agrupar permissões, mover e delegar pelo menu) ganha E2E.

## 4. Melhorias

Cada item tem um ID `UX-nn` para virar card. A coluna "Lado" diz se precisa de backend. Os itens que só mexem na apresentação não criam comando novo. Os que mexem em dados seguem a regra do `CLAUDE.md` e entram no `dispatcher.rs`, com MCP e CLI.

### 4.1 Quadro de Tasks

#### UX-01 · Rolagem por coluna com cabeçalho fixo
- **Proposta.** A página não rola mais. O quadro ocupa a altura que sobra e cada coluna tem rolagem própria. O título da coluna (nome, contagem e traço) fica `sticky` no topo dela. No celular, com uma coluna só, a página continua rolando.
- **Onde.** `TasksPage.vue`: tirar `overflow-y-auto` de `PAGE_CLASSES` e pôr `min-h-0`; `BOARD_AREA_CLASSES` e `GRID_CLASSES` com `min-h-0 h-full`; `COLUMN_CLASSES` com `min-h-0 overflow-y-auto`; o título da coluna com `sticky top-0 bg-bg z-10`.
- **Aceite.** Com 60 cards no Done, rolar o Done não move as outras colunas, e o botão "Novo pedido" continua visível. Ao trocar de projeto, cada coluna volta ao topo.
- **Lado.** Frontend.

#### UX-02 · Done compacto, agrupado e recolhível
- **Proposta.**
  - O card do Done vira uma linha: ID, título truncado, avatar e tempo relativo ("há 2h"). Sem descrição, anexos e barra de progresso.
  - Grupos por data de conclusão: "Hoje", "Esta semana" e "Anteriores (n)", este último recolhido.
  - Só os 10 mais recentes aparecem. O link "Ver todos (60)" abre a lista completa (UX-06).
  - Um botão no título recolhe a coluna inteira numa faixa vertical estreita com o nome e a contagem. A escolha fica em `localStorage`, por projeto.
- **Onde.** `TaskCard.vue` ganha a prop `density: 'full' | 'compact'`; a coluna decide a densidade. O `BacklogItem` não tem data de conclusão. O backend passa a gravar `closedAt` quando o card entra no Done e devolve o campo na listagem.
- **Aceite.** Ao concluir um card, ele entra no topo de "Hoje". Recolher o Done libera a largura para as outras quatro colunas, e a escolha continua depois de reiniciar o app.
- **Lado.** Frontend e backend (`closedAt`, que aparece nas leituras de card do MCP e do CLI sem comando novo).

#### UX-03 · Backlog compacto com triagem em lote
- **Proposta.**
  - Card compacto por padrão: ID, título, 📎 com a contagem de anexos e avatar. A descrição aparece no hover e no foco do teclado, com `line-clamp-3`.
  - O botão "Triar" aparece no hover ou no foco, não em todos os cards.
  - Seleção múltipla com Shift+clique ou caixa de seleção no hover, e uma barra flutuante "Triar 5 selecionados".
- **Onde.** `TaskCard.vue` (densidade, igual ao UX-02); `TasksPage.vue` (estado de seleção); triagem em lote reaproveitando `useTriage`, um item por vez na fila.
- **Aceite.** Com 46 itens, o Backlog cabe em cerca de duas telas. Selecionar 3 e triar cria 3 triagens na fila, e cada card mostra o próprio progresso.
- **Lado.** Frontend. Se a triagem em lote precisar ser atômica, cria um comando `triage_items` exposto.

#### UX-04 · Card sem repetição e conteúdo por coluna
- **Proposta.** Cada coluna mostra o que interessa nela:

  | Coluna | O card mostra |
  |---|---|
  | Backlog | ID, título, anexos (contagem) e ação Triar no hover |
  | Ready | ID, título e o post-it da proposta (resumo, nº de etapas, estimativa), com Iniciar como primário |
  | In progress | ID, título, agente com a atividade atual ("editando useItemPlan.ts"), barra de etapas e custo |
  | In review | ID, título, barra de etapas, custo e o que falta revisar ("2 de 6 etapas feitas") |
  | Done | Linha compacta (UX-02) |

  - Remover o `planLine` quando ele só repete o título. O estado do plano ("proposta aceita") vira um selo pequeno.
  - Os chips "image.png" viram um 📎 com o número. A miniatura fica no modal.
  - A linha "3 micro-tarefas · 0 feitas · 0%" sai. Fica a barra segmentada, com esse texto no `title`.
- **Onde.** `TaskCard.vue`, `useItemPlan.lineFor`, `AttachmentPreviewList`. A atividade atual vem do feed (`useFeed`), que já sabe o que cada agente está fazendo.
- **Aceite.** Nenhum card mostra o mesmo texto duas vezes. Um card do In review com seis etapas mede no máximo ~140px.
- **Lado.** Frontend.

#### UX-05 · Barra de filtros real
- **Proposta.** Uma linha com: busca por texto e ID (atalho `/`), filtro por agente (avatares clicáveis), por tipo (tarefa ou feature), por etiqueta e o atalho "Precisa de mim" (pergunta aberta, proposta para aprovar ou erro). O seletor de densidade "Compacto / Confortável" fica no fim da linha. Os filtros vão para a query da rota, para sobreviver ao recarregar.
- **Onde.** `TasksPage.vue` (`visibleItems`), novos textos em `tasksPageCopy.ts`. Substitui o `UiSegmented` de feature, que vira uma das opções do filtro de etiqueta.
- **Aceite.** Digitar "376" deixa só o `item-376` no quadro. "Precisa de mim" mostra só cards com `questions` sem resposta, proposta pendente ou `error`.
- **Lado.** Frontend. Cobre o `item-26` do backlog.

#### UX-06 · Visão em lista para consulta
- **Proposta.** Um seletor "Quadro / Lista" no cabeçalho. A lista é uma tabela com ID, título, coluna, responsável, etiquetas, custo e data, ordenável e com os mesmos filtros do UX-05. É o destino do "Ver todos" do Done e o jeito de olhar o Backlog grande.
- **Onde.** Componente novo `TaskTable.vue` em `features/backlog/`, que reaproveita `visibleItems` e os rótulos de `backlogLabels.ts`.
- **Aceite.** Ordenar por custo mostra os cards mais caros primeiro, e clicar numa linha abre o mesmo modal (ou painel, UX-08).
- **Lado.** Frontend.

#### UX-07 · Limite de trabalho visível na coluna
- **Proposta.** Quando o [049](backlog/049.md) estiver pronto, o título da coluna mostra `2 / 3` e muda para o tom de aviso ao atingir o limite.
- **Lado.** Depende do 049.

### 4.2 Card aberto

#### UX-08 · Painel lateral no lugar do modal
- **Proposta.** O card abre num painel à direita, com ~560px e largura ajustável, e o quadro continua visível e rolável atrás. `J` e `K` passam para o card seguinte e o anterior da mesma coluna, e `Escape` fecha. Abaixo de `md`, o painel ocupa a tela inteira.
- **Onde.** `TaskItemModal.vue`: trocar `OVERLAY_CLASSES` e `FRAME_CLASSES` por um painel; manter `trapFocus` e devolver o foco ao card que abriu; o `parentStack` (voltar ao item pai) continua.
- **Aceite.** Com o painel aberto, clicar em outro card troca o conteúdo sem fechar. Fechar devolve o foco ao card de origem.
- **Lado.** Frontend.

#### UX-09 · Rodapé com hierarquia de ação
- **Proposta.**

  | Hoje | Proposta |
  |---|---|
  | "Mover para: Backlog · Done" (chips) | Seletor de coluna no cabeçalho, no lugar do selo "Pronto para o time": `● Ready ▾` |
  | "Delegar a" com 7 chips | Botão dividido `Iniciar com Axiom ▾`. A parte principal inicia com o responsável sugerido e a seta abre a lista de agentes com papel e estado ("livre", "ocupado"). Triage só aparece se fizer sentido para o tipo de item |
  | Remover e Arquivar em vermelho no rodapé | Menu `⋯` no cabeçalho, com Arquivar e Remover. O Remover continua passando pelo `UiConfirm` |
  | "Abrir thread" e "Abrir conversas" como texto solto | Links na seção "Ligado a", junto do plano, das branches e do PR |

  O rodapé fica com uma linha só: a dica de contexto à esquerda (ex.: "Os cards entram no kanban do time") e o botão primário à direita.
- **Onde.** `TaskItemModal.vue` (`footer` e `header`), `ActionMenu.vue` ou um `UiMenu` reaproveitável para o `⋯`, `LinkedTo.vue`.
- **Aceite.** No rodapé há no máximo dois controles visíveis. Remover exige dois cliques e confirmação. Iniciar com outro agente leva dois cliques.
- **Lado.** Frontend.

#### UX-10 · Desdobramento legível
- **Proposta.**
  - Renderizar as tarefas e o "Pronto quando" da etapa com `MarkdownText`, para que o código inline saia como código.
  - Uma etapa expandida por vez (acordeão). A primeira etapa ainda não feita abre sozinha.
  - O selo "proposta" sai de cada etapa e fica só no cabeçalho do card. Cada etapa mostra o próprio estado só quando ele é diferente do card (ex.: "feita", "em andamento", "falhou").
  - Ao lado do ícone, o nome do agente da etapa ("Ember"), não só o retrato.
- **Onde.** `PlanBreakdown.vue`, `MicroTaskScript.vue`.
- **Aceite.** No `item-88`, `` `resolveRelatedItem` `` aparece em fonte de código, e o único selo amarelo visível é o do cabeçalho.
- **Lado.** Frontend.

#### UX-11 · Coluna lateral sem repetição
- **Proposta.**
  - "Detalhes" mostra nome e retrato: "Responsável: 🧊 Axiom" e "Aberto por: 🧊 Axiom". Nunca o ID cru do agente.
  - A caixa "Plano" sai quando o desdobramento já está visível. Fica só a estimativa ("~1 dia"), que vai para "Detalhes".
  - "Ligado a" mostra rótulos claros, com ícone: "Plano", "Thread", "Branch `vokia/item-88`", "PR #12".
  - Custo e tempo do card ("$1.19 · 25s"), que já aparecem no quadro, entram em "Detalhes".
- **Onde.** `TaskItemModal.vue` (`details`), `PlanSummaryCard.vue`, `LinkedTo.vue`.
- **Lado.** Frontend.

#### UX-12 · Atividade com texto e tempo humanos
- **Proposta.** Tempo relativo ("ontem, 15:38"), com a data completa no `title`. Corrigir o texto gerado: com o time chamado "Time", escrever "criou o card para o time (líder Axiom)" em vez de "para o time Time".
- **Onde.** `src-tauri/src/db/backlog.rs:1951` e o formatador de data da atividade.
- **Lado.** Backend (texto) e frontend (data).

### 4.3 Conversas

#### UX-13 · Permissões automáticas dentro da atividade do turno
- **Proposta.** Aprovações em bypass ou auto deixam de ser linhas soltas e entram no `ActivityRun` do turno, como linhas de comando. Fechado, o bloco mostra uma frase só: "Axiom rodou 7 comandos (git status, git diff, git commit…) ▸". Aberto, cada linha mostra o comando resumido (`friendlyCommand`) numa linha, e o texto inteiro fica num `UiCommandBlock` sob demanda. Permissão que **pediu** decisão e falha continuam visíveis fora do bloco.
- **Onde.** `MessageEntry.vue` (deixa de mostrar o `permissionResultRow` de aprovação automática), `activitySummary.ts` (frase e agrupamento), `permissionSummary.ts`.
- **Aceite.** No turno do print, as 7 linhas "Modo bypass" viram uma linha recolhida. A resposta "As duas tasks estão finalizadas…" fica visível sem rolar.
- **Lado.** Frontend.

#### UX-14 · Delegação em linha
- **Proposta.** O bloco centralizado vira uma linha alinhada à esquerda, no eixo das mensagens: `↪ Ember delegou ao 🧊 Axiom · "Pedido de implementação: Task…" ▸`. A volta segue o mesmo padrão: `↩ Axiom respondeu ao Ember · "As duas tasks…"`. Expandir mostra o pedido inteiro. A frase "Usou 1 integração" sai quando a única ferramenta do turno for a delegação já mostrada.
- **Onde.** `MessageEntry.vue` (bloco do `→`), `activitySummary.ts` (`DELEGATION_VERB`).
- **Aceite.** Uma ida e volta de delegação ocupa duas linhas de ~20px.
- **Lado.** Frontend.

#### UX-15 · Largura de leitura
- **Proposta.** O texto das mensagens fica limitado a ~80ch (`max-w-[80ch]`). Blocos de código, diffs e tabelas podem passar desse limite até a largura da coluna. As mensagens do usuário continuam à direita, mas a no máximo ~80ch da borda esquerda do texto do agente, para a conversa não virar duas colunas distantes.
- **Onde.** `MessageEntry.vue`, `MarkdownText.vue`.
- **Lado.** Frontend.

#### UX-16 · Threads com estado e agrupamento
- **Proposta.** Cada thread mostra o agente atual (retrato), o card ligado (`item-376`) e um ponto de estado: trabalhando, precisa de você, não lido ou concluída. A lista se divide em "Ativas" e "Concluídas (n)", esta recolhida. A thread de um card que foi para o Done passa sozinha para "Concluídas".
- **Onde.** `ConversasPage.vue`, `useThreads.ts` (já tem `ACTIVE_THREAD_COLUMNS`), regras de `team/status/` (`needsYouRule`, `workingRule`).
- **Aceite.** Com 19 threads e 2 ativas, a lista mostra 2 linhas e "Concluídas (17)".
- **Lado.** Frontend.

#### UX-17 · Agentes ocupados primeiro
- **Proposta.** Os agentes trabalhando aparecem no topo, com a atividade atual ("Axiom · validando item-376"). Os livres viram uma fileira de retratos, com nome e papel no hover. Clicar abre a conversa individual, como hoje.
- **Onde.** `AgentList.vue`, `agentStateStyle.ts`.
- **Lado.** Frontend.

### 4.4 Transversais

#### UX-19 · Cabeçalho de página compacto
- **Proposta.** O título da página fica com ~40px de altura, na linha das ações (filtros e "Novo pedido"). O "Vokia" manuscrito vira o nome do projeto no seletor da sidebar, onde já tem função.
- **Onde.** `UiPageHeader.vue`, `pageLayout.ts`.
- **Lado.** Frontend.

#### UX-20 · Idioma único
- **Proposta.** Rótulos em português em toda a interface: "A fazer", "Pronto", "Em andamento", "Em revisão" e "Concluído". O mesmo rótulo serve para a coluna, o seletor do UX-09 e os filtros. Os valores internos não mudam.
- **Onde.** `tasksPageCopy.ts` (`COLUMN_META`), `backlogLabels.ts` (`COLUMN_CHIP`, `MODAL_STATE_CHIP`).
- **Decisão em aberto.** Se o time prefere os termos de kanban em inglês, o caminho é o contrário: inglês nas colunas e no seletor, sem "Pronto para o time". O que importa é não misturar.
- **Lado.** Frontend.

#### UX-21 · Atalhos de teclado
- **Proposta.** `N` abre um pedido novo, `/` foca a busca, `J` e `K` navegam entre cards, `Enter` abre e `?` mostra a lista de atalhos. Os atalhos ficam desligados quando o foco está num campo de texto.
- **Onde.** Um composable `useShortcuts.ts` em `src/ui/`, com limpeza do listener no `onUnmounted`.
- **Lado.** Frontend.

### 4.5 Tela de Artefatos

#### UX-22 · Card de artefato sem repetição
- **Proposta.** Tira do card o caminho "Artefatos › Plano" e o "0 etapas", e alinha o rodapé de todos os cards.
- **Onde.** `PlanCard.vue`, `ArtifactCard.vue`, `artifactCardClasses.ts`.
- **Lado.** Frontend.

#### UX-23 · Resumo do artefato sem markdown cru
- **Proposta.** O resumo do card mostra texto corrido, sem "#", "##", crases e asteriscos.
- **Onde.** `PlanCard.vue`, `ArtifactCard.vue`; função pura nova, testável em `desktop/tests/*.mjs`.
- **Lado.** Frontend.

#### UX-24 · Estado do plano igual ao do card
- **Proposta.** O selo do plano segue o card ligado: proposta, pronto, em andamento, em revisão ou concluído.
- **Onde.** `PlanCard.vue`, `backlogLabels.ts` (`STATE_CHIP`, `COLUMN_CHIP`), `backlogTypes.ts` (`COLUMN_LABEL`).
- **Lado.** Frontend.

#### UX-25 · Artefatos agrupados por estado e recência
- **Proposta.** A grade separa o que precisa de você, o que está em execução e o que acabou, com os mais recentes primeiro.
- **Onde.** `ArtefatosPage.vue`, `useArtifacts.ts`.
- **Lado.** Frontend.

#### UX-26 · Busca e filtro de estado nos artefatos
- **Proposta.** Campo de busca por título, ID e resumo e filtro por estado, para achar um artefato entre dezenas.
- **Onde.** `ArtefatosPage.vue`.
- **Lado.** Frontend.

#### UX-27 · Abas de tipo no plural e sem contador duplicado
- **Proposta.** As abas dizem "Documentos 2" e "Decisões 1", e o "48 artefatos" repetido sai da linha.
- **Onde.** `ArtefatosPage.vue` (`filterOptions`, `countLabel`), `artifactsCopy.ts`.
- **Lado.** Frontend.

#### UX-28 · Visão em lista dos artefatos
- **Proposta.** Alterna a grade de folhas por uma lista compacta, uma linha por artefato, para consulta.
- **Onde.** `ArtefatosPage.vue`; componente novo de linha.
- **Lado.** Frontend.

#### UX-29 · Plano leva ao card ligado
- **Proposta.** O plano mostra o card ligado e a coluna dele, com um link que abre o card no quadro.
- **Onde.** `PlanCard.vue`, `PlanArtifact.vue`; navegação igual à de `PlanStageRow.vue` (`openCard`).
- **Lado.** Frontend.

## 5. Ordem de execução

1. **Onda 1, rolagem e ruído.** Pequenos e sem dependência, com o maior ganho por linha mudada: UX-01, UX-13 e UX-12.
2. **Onda 2, densidade.** UX-04, UX-02 e UX-03, nesta ordem, porque os três dependem da prop `density` do `TaskCard`.
3. **Onda 3, card aberto.** UX-09, UX-10, UX-11 e depois UX-08. O painel lateral é a mudança maior e fica mais simples depois que o conteúdo já foi enxugado.
4. **Onda 4, conversa.** UX-14, UX-15, UX-16 e UX-17.
5. **Onda 5, navegação.** UX-05, UX-06, UX-19, UX-20, UX-21 e UX-07 (este quando o 049 sair).
6. **Onda 6, artefatos.** UX-22, UX-23 e UX-27 primeiro (pequenos e só de apresentação), depois UX-24, UX-29, UX-25, UX-26 e UX-28.

## 6. Verificação

- **E2E** (no padrão de `desktop/tests/*.mjs`, com um projeto semeado com 60 cards no Done e 46 no Backlog):
  1. Rolar o Done não move o Ready (UX-01).
  2. Recolher o Done sobrevive a reiniciar o app (UX-02).
  3. Um turno com 7 aprovações automáticas mostra um bloco só, que expande e mostra as 7 (UX-13).
  4. Remover pelo menu `⋯` exige confirmação, e Escape cancela (UX-09).
  5. Buscar pelo ID filtra o quadro, e o filtro sobrevive ao recarregar (UX-05).
  6. Um plano de card concluído aparece com o selo "concluído" no grupo recolhido, e a busca pelo ID o encontra (UX-24, UX-25, UX-26).
- **No navegador**, com capturas antes e depois na mesma resolução (1920×1080) e nos temas claro e escuro: densidade dos cards, cabeçalho e largura de leitura.
- **Antes de cada commit:** `cd desktop && npm run typecheck`. Para o UX-12 também `cargo check --bins` e `cargo test --quiet -- --skip live_session`.

## 7. Fora do escopo

- Mudanças no fluxo da orquestração (quem abre o card, regras de camada): estão no [plano de melhorias](plano-de-melhorias.md).
- Tela de diff e revisão: é o [038](backlog/038.md).
- Redesenho visual, como paleta, tipografia e o estilo desenhado à mão: o plano mantém a identidade atual e mexe só em hierarquia, densidade e fluxo.

## 8. Cards

Cada item virou um documento de requisito em [backlog/](backlog/) e um card no backlog do projeto no app. O UX-18 (barra de estado no lugar da Actionbar) saiu em 2026-11-24: partia de uma leitura errada, porque a "pílula" é a própria Actionbar, que fica onde está.

| Item | Requisito |
|---|---|
| UX-01 · Rolagem por coluna com cabeçalho fixo | [054](backlog/054.md) |
| UX-02 · Done compacto, agrupado e recolhível | [055](backlog/055.md) |
| UX-03 · Backlog compacto com triagem em lote | [056](backlog/056.md) |
| UX-04 · Card sem repetição e conteúdo por coluna | [057](backlog/057.md) |
| UX-05 · Barra de filtros real | [058](backlog/058.md) |
| UX-06 · Visão em lista para consulta | [059](backlog/059.md) |
| UX-07 · Limite de trabalho visível na coluna | [060](backlog/060.md) |
| UX-08 · Painel lateral no lugar do modal do card | [061](backlog/061.md) |
| UX-09 · Rodapé do card com hierarquia de ação | [062](backlog/062.md) |
| UX-10 · Desdobramento do plano legível | [063](backlog/063.md) |
| UX-11 · Coluna lateral do card sem repetição | [064](backlog/064.md) |
| UX-12 · Atividade do card com texto e tempo humanos | [065](backlog/065.md) |
| UX-13 · Permissões automáticas dentro da atividade do turno | [066](backlog/066.md) |
| UX-14 · Delegação em linha na conversa | [067](backlog/067.md) |
| UX-15 · Largura de leitura nas mensagens | [068](backlog/068.md) |
| UX-16 · Threads com estado e agrupamento | [069](backlog/069.md) |
| UX-17 · Agentes ocupados primeiro | [070](backlog/070.md) |
| UX-19 · Cabeçalho de página compacto | [072](backlog/072.md) |
| UX-20 · Idioma único na interface | [073](backlog/073.md) |
| UX-21 · Atalhos de teclado | [074](backlog/074.md) |
| UX-22 · Card de artefato sem repetição | [075](backlog/075.md) |
| UX-23 · Resumo do artefato sem markdown cru | [076](backlog/076.md) |
| UX-24 · Estado do plano igual ao do card | [077](backlog/077.md) |
| UX-25 · Artefatos agrupados por estado e recência | [078](backlog/078.md) |
| UX-26 · Busca e filtro de estado nos artefatos | [079](backlog/079.md) |
| UX-27 · Abas de tipo no plural e sem contador duplicado | [080](backlog/080.md) |
| UX-28 · Visão em lista dos artefatos | [081](backlog/081.md) |
| UX-29 · Plano leva ao card ligado | [082](backlog/082.md) |
