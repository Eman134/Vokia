# Plano de melhorias · processo, orquestração e fluxo

Levantamento de 2026-11-24. Compara o Vokia com dois produtos da mesma categoria, o **T3 Code** e o **Orca**, e com o próprio código, para achar o que falta no caminho "do pedido ao merge" (seção 9.1 da [visão de produto](vokia-visao-de-produto.md)). Cada melhoria tem um documento de requisito em [backlog/](backlog/) e um card no backlog do projeto no app.

## 1. Resumo

O Vokia já cobre a parte que nenhum dos dois tem: kanban com plano por etapas, camadas e regras entre agentes, decisões centralizadas e verificação de objetivo. O que falta está quase todo **no fim do ciclo**, depois que o agente escreve código: preparar o worktree, desfazer um turno ruim, revisar o diff com retorno ao agente, abrir o PR e reagir ao CI. Sem isso, o usuário ainda sai do app para fechar cada card.

## 2. Referências comparadas

| Produto | O que é | O que serve de inspiração |
|---|---|---|
| **T3 Code** | GUI para agentes de código (Codex, Claude Code, Cursor, OpenCode e outros), com web, desktop e celular. | Worktree com scripts de preparo (`t3.json`), checkpoint por turno com desfazer, diff em quatro escopos, PR com vigilância de CI ("Watch a pull request"), passagem de contexto entre provedores com teto de tokens, estado da orquestração como eventos em SQLite, fila ou correção do turno em andamento ("Queues" e "Steers"). |
| **Orca** | IDE para rodar vários agentes de CLI em paralelo, cada um no seu worktree, com app de celular. | Comentário na linha do diff que volta ao agente, CI e revisão do PR devolvidos ao agente, aviso "precisa de você", automações agendadas e por gatilho (PR aberto, CI falhou), issues do GitHub e do Linear no mesmo lugar, um agente que divide o trabalho entre outros, CLI com tudo que o app faz. |

Nenhum dos dois tem kanban: o T3 Code tem uma lista de issues e uma caixa de entrada de threads; o Orca mostra as tasks do GitHub e do Linear.

## 3. O que o código do Vokia tem hoje

Conferido em `desktop/` em 2026-11-24.

| Tema | Estado |
|---|---|
| Worktree por card | Existe (`projects/worktree.rs`: criar, garantir, remover, limpar ao concluir). Não roda nenhum preparo depois de criar. |
| Checkpoint, desfazer turno | Não existe. |
| Tela de diff e revisão (seção 8.11) | Não existe; só o bloco de diff dentro da conversa (`UiDiffBlock.vue`). |
| PR e CI | Não existe; há vínculo manual de commit, branch e PR ao card (`link_git_ref`). |
| Notificação do sistema operacional | Não existe; só o ícone na bandeja. Não há `tauri-plugin-notification`. |
| Registro de eventos da orquestração | Não existe; o fluxo vive em memória (`FlowQueue`) com restauração parcial ao abrir. |
| Contexto na troca de provedor | Não existe; o `--resume` só vale dentro do mesmo provedor. |
| Dependência entre cards | Só entre etapas do mesmo plano (`plan_dependencies`). |
| Limite de trabalho em andamento, tempo de ciclo | Não existe. |

## 4. Melhorias

As prioridades seguem o impacto no ciclo de entrega. "Essencial" trava o fluxo do pedido ao merge; "Alto valor" melhora a orquestração por cards; "Depois" não bloqueia nada.

### 4.1 Essencial

| ID | Melhoria | Inspiração | Lado |
|---|---|---|---|
| [036](backlog/036.md) | Preparo do worktree: script de setup e teardown, variáveis e porta por worktree | T3 Code (`t3.json`) | Backend |
| [037](backlog/037.md) | Checkpoint por turno e desfazer a partir da conversa | T3 Code | Backend e usuário |
| [038](backlog/038.md) | Diff do card com comentário na linha que vira pedido ao agente | Orca e T3 Code | Usuário |
| [039](backlog/039.md) | Abrir PR pelo card e vigiar o CI, devolvendo falhas ao agente | T3 Code ("Watch a pull request") e Orca | Backend |
| [040](backlog/040.md) | Notificação nativa do sistema para decisões, revisão e CI | Orca e T3 Code ("Needs you") | Usuário |

### 4.2 Alto valor

| ID | Melhoria | Inspiração | Lado |
|---|---|---|---|
| [041](backlog/041.md) | Registro de eventos da orquestração por card | T3 Code (eventos em SQLite) | Backend |
| [042](backlog/042.md) | Contexto levado ao trocar de provedor, com teto de tokens | T3 Code (handoff) | Backend |
| [043](backlog/043.md) | Dependência entre cards ("bloqueado por") | Kanban | Backend e usuário |
| [044](backlog/044.md) | Estado do git no card do kanban: branch, diff, PR, CI | T3 Code (estado do PR no thread) | Usuário |
| [045](backlog/045.md) | Corrigir o rumo do turno em andamento, além da fila | T3 Code ("Queues" e "Steers") | Usuário |
| [050](backlog/050.md) | Separar decisão e efeito na orquestração | T3 Code (AGENTS.md) | Backend |

### 4.3 Depois

| ID | Melhoria | Inspiração | Lado |
|---|---|---|---|
| [046](backlog/046.md) | Rodar a mesma tarefa em dois agentes e escolher o melhor | Orca | Backend e usuário |
| [047](backlog/047.md) | Automações agendadas e por gatilho | Orca (Bot Schedules & Triggers) | Backend |
| [048](backlog/048.md) | Issues do GitHub e do Linear no backlog | Orca e T3 Code | Backend e usuário |
| [049](backlog/049.md) | Limite de trabalho em andamento e tempo de ciclo | Kanban | Usuário |
| [051](backlog/051.md) | Acesso remoto e celular | Orca e T3 Code | Backend e usuário |

### 4.4 Restos da otimização de tokens

Da revisão de consumo de 2026-11-24 (commit `1e74b64`), ficaram dois pontos pequenos de código.

| ID | Melhoria | Lado |
|---|---|---|
| [052](backlog/052.md) | Instruções da raiz do projeto já no prompt do agente | Backend |
| [053](backlog/053.md) | Limpar os avisos de código morto do build | Backend |

Ficam como configuração, sem card: Ember em Haiku e avaliar Sonnet no lugar de Opus para a triagem.

## 5. Ordem de execução

1. **Onda rápida** (pequenos e sem dependência): 053, 052, 040 e 036.
2. **Fechar o ciclo de entrega**, nesta ordem, porque cada um usa o anterior: 037 (desfazer dá segurança para o resto), 038 (revisão), 039 (PR e CI), 044 (o kanban mostra tudo isso).
3. **Robustez da orquestração**: 050, depois 041 (fica simples com decisão separada de efeito), 042, 043 e 045.
4. **Depois**: 049, 048, 047, 046 e 051.

## 6. Princípios que vêm junto

- **Tudo que o app faz, o agente também faz.** Cada melhoria entra no catálogo de comandos (`commands/dispatcher.rs`), fica disponível pelo MCP e pelo `vokia-cli`, como manda o `CLAUDE.md` do projeto. O Orca segue a mesma regra ("Anything you can do in Orca, your agents can do too").
- **Complexidade na borda do adaptador.** A orquestração decide, os adaptadores executam, a interface só mostra ("Orchestration stays pure, UI stays dumb", do AGENTS.md do T3 Code). É a base do item 050.
- **Cada ida à API custa o contexto inteiro.** Toda melhoria que acorda um agente (CI, gatilhos, reenvios) tem limite de tentativas e respeita a pausa por limite de uso.

## 7. Fontes

- [stablyai/orca · README](https://raw.githubusercontent.com/stablyai/orca/HEAD/README.md) e [repositório](https://github.com/stablyai/orca).
- [A deep dive into T3 Code](https://flaviocopes.com/t3-code) (Flavio Copes, versão 0.1.11).
- [T3 Code · AGENTS.md](https://raw.githubusercontent.com/pingdotgg/t3code/main/AGENTS.md).
- [T3 Code guide](https://betterstack.com/community/guides/ai/t3-code/) (Better Stack, versão mais antiga).
