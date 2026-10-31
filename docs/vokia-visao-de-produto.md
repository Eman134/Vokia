# Vokia · Visão de produto

> O nome é **Vokia**, e a marca é uma **chama**. O projeto começou como **devflow** (e passou por flowcode); código, pastas e branches agora usam `vokia`.
> Este documento descreve o que está desenhado no canvas de telas e como cada parte deve funcionar. Atualizado em 25/10/2026.

---

## 1. Em uma frase

Um app desktop open source onde você monta **times de agentes de IA**, organiza o trabalho deles em **camadas** (quem planeja, quem implementa, quem revisa) e acompanha tudo em **vários projetos ao mesmo tempo**. Os agentes rodam pelas CLIs oficiais (Claude Code e Codex), no seu computador e com o seu login.

## 2. O problema

- Quem usa agentes de código hoje abre vários terminais, um por tarefa, e perde o fio: quem está fazendo o quê, o que está parado esperando uma resposta, o que já foi revisado.
- Os agentes não conversam entre si de forma organizada. Revisão, testes e pesquisa dependem de você copiar e colar entre sessões.
- Planos, specs, diagramas e decisões ficam espalhados em arquivos soltos e conversas que somem.
- Não existe uma visão única de vários projetos, nem um jeito simples de aprovar comandos, responder perguntas e liberar merges sem trocar de janela.

## 3. A proposta

1. **Uma tela por pergunta.** "O que está acontecendo?" é o Mapa do time. "O que falta e o que anda?" é o Backlog. "O que eles estão dizendo e rodando?" são as Conversas. "O que mudou?" é o Diff e revisão. Pedir algo é sempre pela barra de ação.
2. **Uma barra de ação em todas as telas**, fixa no centro inferior, que mostra o estado geral, recebe pedidos, guia fluxos em etapas e abre para você decidir sem sair de onde está.
3. **Orquestração visual.** Você desenha o fluxo entre as camadas com setas e regras ("quando a revisão reprovar, devolva para a implementação, no máximo 2 vezes").
4. **Tudo vira arquivo no repositório.** Planos, specs, diagramas, ADRs e memória ficam versionados no worktree. Se a sessão cair, o trabalho continua.
5. **Você no controle.** Comandos sensíveis, planos e merges param e esperam a sua aprovação.

## 4. Princípios

- **Simples primeiro.** Uma solução clara, sem caminhos alternativos escondidos.
- **Falhar cedo.** Pré-condições não atendidas viram erro visível, nunca um comportamento silencioso.
- **Nada de credenciais.** O app nunca lê, guarda nem repassa logins. O usuário entra nas CLIs oficiais, e o app apenas as executa.
- **Evidência antes de "pronto".** Nenhuma tarefa é concluída sem rodar os comandos de verificação e mostrar a saída.
- **Estado legível sem cor.** Todo estado (trabalhando, esperando, concluído, falha, pausado) tem forma própria, não só cor.
- **Sempre há "Outro…".** Toda pergunta ou múltipla escolha que chega para você traz a opção "Outro…" com campo de texto livre.

## 4.1 Nome e marca

- **Nome:** Vokia.
- **Logo:** uma chama de traço cheio cuja base forma a letra **V**. Funciona sozinha a partir de 16 px, e ao lado de "Vokia" em Geist semibold, nas versões vertical e horizontal.
- **Ícone do app:** a chama clara sobre um quadrado arredondado escuro, e a versão invertida.
- **Cores:** preto e branco como base; o gradiente laranja (#f08a3c → #e0524a) só no ícone e no splash.
- **Mascote:** a mesma chama com rosto, braços e pernas. Aparece no onboarding, em estados vazios, em avisos de atualização e em telas de erro. Não substitui os avatares dos agentes.

| Estado do app | Como a chama aparece |
|---|---|
| Trabalhando | Chama alta e acesa, olhos abertos |
| Precisa de você | Chama piscando, acenando |
| Tudo em dia | Chama pequena, sorrindo |
| Erro | Chama curvada, com um fio de fumaça |
| Pausado | Brasa, olhos fechados |
| Atualização pronta | Chama segurando o notebook |

- **Onde aparece:** topo do trilho, barra de título e splash, ícone do executável e da bandeja, site do projeto.

---

## 5. Conceitos e modelo de dados

| Conceito | O que é | Exemplo nas telas |
|---|---|---|
| **Workspace** | Tudo o que é seu: projetos, agentes, skills, configurações. | Trilho com Projetos, Agentes, Estatísticas, Skills |
| **Projeto** | Um repositório local com símbolo, times e histórico próprios. | loja-online, app-agenda, site-estudio, vokia (pausado) |
| **Feature** | Uma entrega maior dentro de um projeto, com branch e worktree próprios. | Pagamento via Pix, Cartão parcelado |
| **Item / Task** | Unidade de trabalho do kanban do backlog, com etiquetas e uma coluna (Backlog, Ready, In progress, In review, Done). | PIX-4 · Webhook do PSP |
| **Pedido** | Tudo o que você pede ao projeto, por uma entrada única. Pequeno vai direto para um agente, sem card; feature vira card pai e passa pela triagem da líder. | "Quero frete grátis por região" |
| **Micro-tarefa** | Pedaço pequeno de uma feature, com agente e modelo próprios, de preferência um modelo mais barato. | PIX-4 · Webhook do PSP · GPT-5 mini |
| **Etiqueta** | Marcador do item que também filtra o kanban. | bug, urgente, interface, backend, ideia, dívida técnica, pedido de cliente |
| **Plano** | Documento com etapas, lotes e passos. Pode ser de feature, task ou bug, e uma feature pode ter sub-planos de task. | Plano · Cartão parcelado → sub-plano PAY-13 |
| **Lote** | Grupo de passos executados juntos, com ponto de revisão no fim. | Lote 2 · Parcelas no checkout |
| **Agente** | Uma instância de CLI (Claude Code ou Codex) com nome, foto, role, skills e modelo. | Iara, Davi, Nina, Léo, Téo, Rui, Bia |
| **Role** | O papel e as instruções de um agente. | Tech Lead, Frontend, Backend, Revisor, Pesquisa, Designer |
| **Camada** | Uma etapa do fluxo de trabalho, ocupada por agentes com certas roles. | Liderança, Implementação, Revisão (e novas: Pesquisa, QA, Segurança, Documentação) |
| **Time** | Um grupo de agentes que trabalha junto, cruzando camadas. | Web, Pagamentos, Qualidade, Pesquisa |
| **Orquestração** | A camada que decide as etapas e as regras das setas entre camadas. | Iara conduz, etapas por lote, até 4 agentes em paralelo |
| **Regra (seta)** | "Quando X acontecer na camada A, faça Y na camada B". | Quando a revisão reprovar → devolver, até 2x |
| **Decisão** | Algo que espera você: permissão, pergunta, escolha de caminho, revisão de lote, memória nova. | Nina quer rodar `npm install qrcode@1.5.4` |
| **Fila** | Pedidos esperando um agente específico. | Davi: 1. Seletor de parcelas, 2. Corrigir role="alert" |
| **Artefato** | Conteúdo durável do projeto: whiteboard, diagrama, documento, caso de uso, anotação, ADR. | Fluxo do pagamento Pix (Mermaid) |
| **Memória** | Fatos e decisões que os agentes leem antes de trabalhar. | "txid chega em maiúsculas; o front normaliza" |
| **Skill** | Instruções reutilizáveis para um tipo de tarefa. | TDD, systematic-debugging, write-plan |
| **Limite de uso** | A janela de 5 h e a semana de cada plano (Claude e ChatGPT). A Vokia mede, avisa e pausa o time antes do limite. | Claude em 78% da janela de 5 h |

Relações principais:

```
Workspace
 └─ Projeto ─┬─ Features ─┬─ Planos (feature) ─┬─ Lotes ─ Passos
             │            │                    └─ Sub-planos (task)
             │            └─ Itens do backlog (coluna, etiquetas, plano)
             ├─ Times ── Agentes ── Role, Skills, Modelo, Fila
             ├─ Camadas + Regras (orquestração)
             ├─ Artefatos
             └─ Memória
```

---

## 6. Como funciona por baixo

### 6.1 Execução dos agentes

- Cada agente é um processo da **CLI oficial** rodando em modo headless:
  - **Claude Code:** `claude -p --input-format stream-json --output-format stream-json --verbose --include-partial-messages`, com `--append-system-prompt` (role), `--mcp-config` (servidor da plataforma), `--allowedTools`, `--permission-mode` e `--permission-prompt-tool` (as permissões viram decisões na barra), e `--resume` para continuar sessões.
  - **Codex:** a CLI oficial com login do ChatGPT, no mesmo modelo de processo.
- O usuário faz login nas CLIs por conta própria (assinatura ou chave). O app só detecta se a CLI está instalada e logada, na tela de Configurações.
- A saída em stream-json alimenta as Conversas: mensagens, ferramentas usadas, comandos e saídas de terminal.

### 6.2 Isolamento por feature

- Cada feature ganha um **git worktree** e uma branch `vokia/<slug>`.
- Agentes da mesma feature trabalham no mesmo worktree, em passos que não colidem. O grafo de código avisa quando dois itens mexem no mesmo arquivo (ex.: `payments/pix.ts` aberto em PIX-3 e PAY-12).

### 6.3 Servidor MCP da plataforma

Os agentes falam entre si e com você por um servidor MCP local, com ferramentas como:

| Ferramenta | Para quê |
|---|---|
| `delegate_task` | Um agente passa trabalho para outro, respeitando as regras da orquestração |
| `report_result` | Entrega o resultado de um passo com evidências |
| `ask_human` | Cria uma decisão na barra de ação (pergunta ou escolha de caminho) |
| `get_feature_context` | Lê plano, spec, artefatos e memória ligados à feature |
| `post_note` | Escreve na conversa do projeto ou nas anotações |
| `approve` | Fluxo de aprovação de lote ou plano |

### 6.4 Persistência

- Banco local **SQLite** para estado do app: projetos, agentes, filas, decisões, histórico.
- Arquivos no repositório (`.vokia/` e `docs/`) para o que precisa ser versionado: `PLAN.md`, `progress.md`, `verification.md`, specs, ADRs, diagramas `.mmd` / `.puml`, memória.

### 6.5 Stack do app

Tauri 2 (Rust no backend, com comandos e eventos), Vue 3 com script setup, TypeScript estrito, Vite, Pinia e Tailwind 4.

### 6.6 Distribuição e atualização

O app é **todo local**: roda, guarda dados e executa os agentes só na máquina. A única chamada externa do próprio app é a checagem de atualização ao abrir, feita pelo lado nativo do app e sem enviar dados do usuário.

- **Plataforma atual:** Windows x64, com instalador NSIS (`.exe`) que instala por usuário, sem administrador, em modo passivo com barra de progresso. macOS e Linux ainda não são publicados. Quando macOS entrar, o app só avisará sobre a versão nova sem assinatura Apple, como a regra atual.
- **Gatilho:** o app só enxerga uma **release publicada** no GitHub, nunca rascunho nem pré-release, cujo tag seja `vX.Y.Z` e cuja versão seja maior que a instalada, comparada por versão semântica. Commits, pushes na branch principal e mudanças em `desktop/` não disparam nada por si só.
- **Ao abrir:** em build de produção, o app consulta o `latest.json` anexado à release mais recente do repositório público `Eman134/Vokia`. Se houver versão nova, abre um pedido de atualização em modal, com versão, data e notas de versão. "Atualizar agora" baixa com progresso, verifica a assinatura, instala e reabre o app. "Depois" fecha o pedido só nesta abertura; na próxima, o pedido volta. A checagem não roda no modo de desenvolvimento.
- **Notas de versão:** o texto exibido é a mensagem do tag anotado `vX.Y.Z`, que a Action usa como corpo da release e do `latest.json`. Editar as notas no rascunho da release depois do build **não** altera o `latest.json`. As notas aparecem como texto puro, com quebras de linha preservadas e rolagem quando são longas.
- **Como publicar uma versão:**
  1. Suba a versão em `desktop/package.json`, que é a fonte única do número do app. O `Cargo.toml` deve receber o mesmo número.
  2. Faça commit dessa mudança na branch principal.
  3. Crie o tag anotado `vX.Y.Z` com as notas de versão como mensagem e envie o tag ao GitHub. Se o número do tag não bater com o `package.json`, a Action falha antes de publicar qualquer coisa.
  4. Acompanhe a Action de release no GitHub. Ela compila no Windows, assina o instalador e cria a release **como rascunho**, com o instalador NSIS, o arquivo de assinatura e o `latest.json`.
  5. Confira o rascunho (instalador anexado, versão e notas) e clique em **Publicar**. Só depois disso as instalações existentes enxergam a versão nova. Um rascunho esquecido significa que ninguém recebe a atualização.
- **Chaves de assinatura:** o par de chaves do Tauri é gerado uma vez, fica fora do repositório, na pasta `.tauri` do diretório do usuário, e tem senha. A chave privada e a senha ficam **somente** no gerenciador de senhas de quem publica e nos secrets do GitHub Actions, com os nomes `TAURI_SIGNING_PRIVATE_KEY` (conteúdo do arquivo da chave privada) e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Nunca commite a chave privada nem arquivos `.key`. A chave pública fica em `tauri.conf.json`, porque é pública por definição. Se a chave privada ou a senha se perderem, as instalações existentes ficam presas na versão atual para sempre.
- **Falhas:** erro de rede, resposta 404, JSON inválido ou assinatura inválida aparecem como aviso discreto e dispensável. O app segue na versão instalada e nunca trava. A checagem tem limite de 10 segundos e roda depois que a tela principal carrega, sem atrasar a abertura.
- **Windows sem assinatura de código:** o SmartScreen pode alertar na primeira instalação. As atualizações seguintes são feitas pelo próprio app, sem passar pelo navegador.
- **App na bandeja:** o app que fica na bandeja do sistema só checa atualização ao abrir. Dias na bandeja significam nenhuma nova checagem.
- **Modo desenvolvimento** (quem contribui): rodar direto do repositório com `git pull`, `npm install` e `npm run tauri:dev` dentro de `desktop/`. Não é o caminho para usuários finais, e a checagem de atualização não roda nesse modo.
- **Configurações (ainda não implementado):** a versão instalada na tela, "Procurar atualização agora" e um interruptor para desligar a busca ao iniciar.

---

## 7. Navegação

### 7.1 Trilho de ícones (esquerda)

- **Recolhido (88 px):** só ícones, em grupos flutuantes. O item ativo tem fundo violeta claro.
- **Expandido (232 px):** mesmo trilho com o nome de cada página. Abre e fecha pelo botão "Recolher" no rodapé, e a escolha fica salva por pessoa.
- **Muda conforme a página:**
  - Dentro de um projeto: botão do projeto atual (trocar de projeto), Mapa do time, Backlog e planos, Conversas, Plano, Artefatos, Diff e revisão, Memória e grafo.
  - No workspace: atalhos para cada projeto com ponto de estado ("loja-online · 5 trabalhando") e "Novo projeto".
  - Sempre: Projetos, Agentes e camadas, Estatísticas, Skills, Configurações e o avatar do perfil.
- **Medidor de uso no avatar:** dois anéis em volta da foto, o de fora para o Claude e o de dentro para o GPT, mostrando a janela de 5 h de cada plano (ver 7.3).
- Saíram do trilho: "Chat e kanban" (o kanban agora é o Backlog e os pedidos vão pela barra) e "Aprovações" (as decisões ficam na barra de ação e nas Conversas).

### 7.2 Barra de ação (inferior)

A barra está em **todas as telas**, inclusive no onboarding e no novo projeto. Fica fixa na tela, centralizada na parte inferior, a 24 px da borda, e não rola com a página.

- **Compacta (sem o mouse em cima):** anel de progresso, status curto e contador de decisões. Quando não há pendências, o contador vira ✓.
- **Expandida (mouse em cima ou campo em foco):** status e detalhe, **menu ☰** com as ações da página (abre para cima, com atalhos ⌥1, ⌥2), campo de pedido e botão **decisões**, que abre o painel da barra (⌘J). "Nova feature" e "Novo pedido" no menu abrem o compositor de pedido por cima da página. Nunca quebra linha; textos longos viram reticências.
- **Modo** configurável: automática (abre no hover ou foco), sempre compacta ou sempre aberta.
- **Fora do app** (Configurações › Barra de ação e avisos): com a Vokia minimizada, a barra continua no sistema operacional, em um de três modos:
  - **Ícone na bandeja:** a chama com anel de progresso e ponto laranja quando há decisão. O clique abre um painel com o status, os agentes e o que cada um faz, a decisão mais urgente com Permitir/Negar, os limites de 5 h, o campo de pedido e "Abrir Vokia".
  - **Na barra de tarefas:** um widget fixo ao lado da bandeja, com anel, "4 trabalhando", o agente em destaque, rostos empilhados e o contador de decisões. As novidades chegam como notificação do sistema. Fora do app só dá para aprovar comandos; uma proposta de feature abre com "Ver proposta".
  - **Lateral da tela:** uma faixa fina presa na borda direita, sempre visível, com o anel, os agentes com o estado de cada um e o contador. O clique abre o painel ao lado.
  - O ícone do app na barra de tarefas também mostra o anel de progresso e o contador de decisões.
- **Campo de pedido:** é onde você pede algo ao projeto. `@` menciona um agente ou projeto, `/` abre comandos (`/plano`, `/revisar`, `/pausar`, `/lembrar`).
- **Estados**, com uma ordem de prioridade (o mais importante vence):
  1. **Erro ou pausado:** anel vermelho, ex.: "Testes falharam" ou "Pausado pelo limite".
  2. **Precisa de você:** anel laranja, contador laranja.
  3. **Trabalhando:** anel verde com progresso.
  4. **Repouso:** "Tudo em dia".
- **Fluxos em etapas:** no onboarding (6 etapas: Boas-vindas, Conexões, Projeto, Time, Pedido, Pronto) e no novo projeto (3 etapas: Origem, Time, Pedido), a barra troca o menu e o botão de decisões por chips numerados. A etapa atual aparece com o nome, as feitas com ✓, e o anel mostra o progresso do fluxo ("Onboarding · 2/6"). Cada chip leva à sua etapa.
- **Avisos (balão acima da barra):** aparecem só em momentos que pedem atenção, com uma ação principal e uma secundária:
  - **Limite (75%):** "Claude em 78% da janela de 5 h", com "Modo econômico" ou "Agora não".
  - **Pausado (90%):** "Time pausado pelo limite", com "Continuar com o Codex" ou "Esperar".
  - **Renovou:** "Janela renovada, o time voltou".
  - **Pedido:** "Iara está fazendo a triagem" quando um pedido entra, e "Reembolso parcial desdobrado em 4 micro-tarefas" com Aprovar quando a proposta chega.
  - **Momentos** do time, ligados ou desligados em Configurações: memória nova, grafo, decisão que vira ADR, skill sugerida, conflito entre agentes e feature concluída. Sem resposta, o balão some e a sugestão vai para Memória › Sugestões.
- **Variações:** em paralelo (rostos empilhados e um traço por agente), decisão em linha (pedido de um clique direto na barra), concluído, mencionando, comando, compacta para telas cheias.
- **Painel (⌘J abre, Esc fecha):**
  - **Decisões:** permissões de comando, escolha entre caminhos (com estimativa), perguntas com respostas prontas, revisão de lote, memória nova. Cada decisão resolvida vai para "Resolvidas agora", com Desfazer.
  - **Acontecendo agora:** inclui "Pedidos rápidos" (os pequenos, sem card, com a posição na fila) até virarem commit. Subaba por agente, com a tarefa atual, as etapas (feitas, atual, esperando você, próximas), as últimas ações e comandos, a fila, os números do dia e com o que ele roda (CLI, modelo, worktree). Na subaba "Todos", a lista de agentes com filtro por projeto e Pausar/Retomar.
- **Ações por página (menu ☰):**

| Página | Ações |
|---|---|
| Projetos | Novo projeto |
| Mapa do time | Pausar time, Nova feature |
| Backlog e planos | Novo pedido, Triar pendentes |
| Conversas | Nova conversa, Pausar todos |
| Plano | Revisar lote, Pausar plano |
| Artefatos | Novo artefato, Gerar cards |
| Memória | Revisar fatos, Buscar na memória |
| Agentes | Novo agente, Enhance |
| Estatísticas | Exportar |
| Skills | Nova skill, Buscar skill |
| Diff e revisão | Pedir correção, Abrir PR |
| Configurações | Verificar CLIs |

### 7.3 Limites de uso dos planos

O custo é dos planos do usuário (Claude e ChatGPT), que têm uma janela de 5 h e um limite semanal. A Vokia não paga nem intermedia nada: ela mede o uso, avisa e segura o time antes de estourar.

- **No trilho (sempre visível):** os anéis no avatar. Neutros no normal, laranja com um ponto de alerta acima de 75%, vermelho quando o time pausou. Expandido, o trilho mostra uma barrinha de 5 h por plano.
- **Painel do avatar (clique):** por plano, a janela de 5 h e a semana em %, quando renova e a previsão no ritmo atual ("acaba em ~25 min"), com links para Estatísticas e para Configurações › Uso e limites.
- **Na barra de ação (só nos limiares):** os avisos de 75% e 90% e o de janela renovada (7.2).
- **Regras:** avisa em 75% e pausa em 90% da janela de 5 h (ajustável). Ao pausar, o time retoma sozinho quando a janela renova, ou você continua com o outro CLI. O **modo econômico** passa a liderança para Sonnet e usa o grafo para cortar o contexto.
- **Em Estatísticas:** a seção "Uso do plano" com o histórico (8.9).

---

## 8. Telas

### 8.1 Projetos

- Cartões de cada projeto com símbolo, caminho, features e progresso.
- **Estado do projeto** em três blocos: **Trabalhando** (quem faz o quê), **Pendente** (o que espera você) e **Em seguida** (próxima tarefa).
- **Mini barra de ação** por cartão: Pausar, contador de pendências e Abrir.
- Projeto pausado (vokia) com "Montar time" e "Retomar".
- O que precisa de você em todos os projetos fica no painel de decisões da barra de ação, não numa faixa da página.
- **+ Novo projeto** abre o fluxo de 3 etapas (8.13).

### 8.2 Mapa do time

Duas visões:

- **Times e camadas:** uma linha por time, uma coluna por camada. Cada cartão de agente mostra estado e tarefa. A tarefa anda para a direita e volta quando a revisão reprova. Ao lado ficam os movimentos recentes e um resumo de "como o fluxo anda".
- **Orquestração:**
  - As camadas aparecem como blocos ligados por setas, com "Você" no fim (aprova o merge).
  - A **camada de orquestração** fica no topo, ligada a todas. Ela define quem conduz (ex.: Iara), as etapas (por lotes ou livre), quando parar para você (a cada lote, só no fim, nunca), quantos agentes trabalham em paralelo e a ordem das etapas.
  - **Cada seta é uma regra** editável:
    - Quando: plano aprovado, tarefa concluída, revisão reprovar, testes falharem, revisão aprovar, agente pedir ajuda.
    - Então: delegar, pedir revisão, devolver, chamar você, pesquisar antes.
    - Execução: uma por vez ou em paralelo.
    - Limite de devoluções antes de chamar você.
    - Uma frase resume a regra.
  - **+ Seta** liga quaisquer duas camadas. **+ Camada** (Pesquisa, QA, Segurança, Documentação) entra antes de "Você" já ligada. **+ Time** (Mobile, Dados, Infra) vira uma linha nova.
- Prancheta própria: "Mapa do time · Orquestração".

### 8.3 Backlog e planos

O backlog é o **kanban do projeto** e o destino de todo pedido. Cada item anda da esquerda para a direita: ganha um plano no Backlog, o time executa, você revisa e ele é entregue.

**Pedidos (entrada única).** "+ Novo pedido" no Backlog, "Nova feature" no Mapa e o campo da barra abrem o mesmo compositor (`PedidoNovo`):

- **Virar pedido de qualquer lugar:** uma linha comentada no Diff, uma mensagem nas Conversas ou um trecho selecionado num artefato abre o compositor já com a origem ("Vem de Diff · src/checkout/PixStatus.tsx · linha 14") e o trecho, que vão junto para quem pegar o pedido.
- **Pequeno** (ajuste, bug, pergunta): vai direto para a fila de um agente escolhido, **sem card**. O rastro fica no commit e no PR, e enquanto isso ele aparece em "Pedidos rápidos" no painel da barra. Se o agente perceber que é maior, a barra avisa "Davi acha que isso é maior" com **Mandar para a triagem**.
- **Feature:** vira um **card pai** no Backlog, já com o estado "triagem". A **líder do time sempre faz a triagem** e orquestra o mini time:
  1. lê o código, a memória e os artefatos;
  2. pergunta o que faltar (decisões na barra e no card, com respostas prontas);
  3. desdobra a feature em **micro-tarefas**, cada uma com agente, modelo e um **roteiro fechado**, porque modelos leves executam e não decidem:
     - **Recebe de contexto:** os fatos da memória, ADRs e o que as micro-tarefas anteriores entregaram.
     - **Pode mexer só em:** a lista exata de arquivos.
     - **Passos:** numerados e concretos, começando pelo teste que falha quando faz sentido.
     - **Pronto quando:** o comando que precisa passar (ex.: `npm test -- refund`).
     - **Não pode:** os limites (não mexer no gateway, não criar dependência…).
     - Depois de uma devolução, o roteiro vira **v2 · mais detalhado**, mostrando o que a líder acrescentou.
     - Se algo fugir do roteiro, o modelo para e devolve para a líder em vez de improvisar. O roteiro aparece no modal da feature, ao abrir cada micro-tarefa. As simples vão para modelos mais baratos (Haiku, GPT-5 mini). Se uma volta da revisão, a líder primeiro detalha o roteiro e só na segunda vez sobe o modelo (9.1);
  4. você aprova a proposta, as micro-tarefas vão para Ready e a branch é criada quando a primeira começa.
- **Acompanhar:** pelos balões da barra ("triagem", "proposta"), pelo estado no card pai ("triagem", "esperando você", "4 micro-tarefas · 1 feita") e pelos filtros do kanban.

- **Colunas:**

| Coluna | O que significa |
|---|---|
| **Backlog** | Ideias e pedidos. É aqui que o item ganha um plano. |
| **Ready** | Plano aprovado, esperando o time. |
| **In progress** | Um agente está trabalhando. |
| **In review** | O diff está esperando revisão. |
| **Done** | Entregue nesta semana. |

- **Um ciclo só:** todo item da coluna Backlog passa pela triagem da líder: não triado → triagem → proposta → Ready. "Triar os N pendentes" manda de uma vez os que ainda não foram triados. Vincular um plano que já existe continua possível.
- **Densidade:** Confortável ou Compacta no topo do Backlog; o padrão vem de Configurações › Geral. A compacta mostra código, título, rosto, estado e só o que importa (feature, n/total feitas, passo da triagem, etiquetas como pontos, barra fina de progresso).
- **Coluna do card pai:** vai para In progress quando a primeira micro-tarefa começa, para In review quando todas as que faltam estão em revisão, e para Done quando todas terminam.
- **Mover à mão:** só entre Backlog, Ready e Done. In progress e In review andam sozinhos com os agentes e o git.
- **Filtros:** busca, feature (Pix, Painel, Checkout, Cartão), tipo (Tudo, Features, Micro-tarefas), "Meus pedidos" e etiquetas. O resumo mostra quantos itens estão em triagem, não triados, com proposta esperando você e em revisão.
- **Card do item:** código, etiquetas no topo à direita, título, descrição curta, quem está com o item e um chip de estado. Sem nome de feature nem de modelo no card; o modelo e o roteiro aparecem só no modal. O **card pai** de feature tem o selo "feature" e aparência empilhada, mostra a triagem em três passos (lendo, perguntando, desdobrando) e, depois, a barra segmentada das micro-tarefas. A **micro-tarefa** mostra "↳ pai". O bloco do meio muda conforme a coluna:
  - No **Backlog**: *não triado* (botão "Triar" e um ícone para vincular um plano existente), *triagem* (passos da líder), *proposta* (Revisar), *pronto*.
  - **Ready:** plano vinculado com o progresso.
  - **In progress:** o lote ("Lote 2 de 3"), com barra.
  - **In review:** quem revisa e o tamanho do diff, com "Ver diff".
  - **Done:** barra cheia, "feito" e, no modal, "Ver replay".
- **Etiquetas:** bug, urgente, interface, backend, ideia, dívida técnica e pedido de cliente, cada uma com sua cor. São editáveis no modal e filtram o kanban.
- **Modal do item** (clique no card): na feature, as perguntas da líder durante a triagem e o desdobramento (micro-tarefa, agente, modelo, estado); em todos, "Conversa com {dono}", etiquetas editáveis, descrição, critérios de aceite (checklist), atividade com campo de comentário e @agente, detalhes, resumo do plano, ligações (plano, artefatos, branch), "Mover para" (só Backlog, Ready e Done; In progress e In review andam sozinhos) e a ação principal do estado.

**Componente Plano do item** (usado no backlog, nos cards e nas conversas), com 4 estados:

1. **Sem plano:** vincular um plano existente ou pedir a um agente, escolhendo quem planeja, a profundidade (rápido ou completo com pesquisa) e o que ele usa (memória, grafo, skill write-plan, revisão antes de executar).
2. **Agente escrevendo:** etapas do agente em andamento.
3. **Proposta:** lotes com responsável e tempo, riscos, e os botões Aceitar e vincular, Ajustar e Descartar.
4. **Vinculado:** plano com progresso, Abrir plano, Desvincular.


### 8.3.1 Replay de uma feature entregue

Toda feature em Done tem um replay, aberto pelo modal do card ("Ver replay") ou pelo cartão do projeto.

- **Resumo:** tempo do pedido ao merge, micro-tarefas, devoluções, escaladas de modelo, diff, testes e uso do plano.
- **Linha do tempo navegável** (‹ › e marcadores coloridos por tipo), com os eventos por fase: pedido e triagem, proposta, execução, revisão, verificação e entrega.
- **Detalhe do evento:** perguntas e respostas da triagem, o plano proposto, o diff de cada commit, o comentário da revisão, o roteiro v2, a saída dos testes. Atalhos para a conversa, o Diff e "Virar pedido".
- **Embaixo:** a tabela das micro-tarefas (agente, modelo, commits, diff, devoluções, tempo), os arquivos alterados e o que o time aprendeu.

### 8.4 Planos

- Lista de **todos os planos do projeto**, agrupada por feature, task e bug, com estado (executando, em revisão, rascunho, concluído, escrevendo, aprovar). Planos de task aparecem aninhados na feature.
- Para o plano selecionado:
  - **Etapas:** Brainstorm, Especificação, Plano, Execução em lotes, Verificação, Entrega.
  - **Lote e micro-tarefa:** cada passo do plano é uma micro-tarefa, com agente e modelo; um lote é um grupo de micro-tarefas revisado junto.
  - **Lotes e passos**, com quem faz e o código do item. Um passo com plano próprio mostra o atalho para o sub-plano.
  - **Critérios de verificação:** comandos que precisam rodar com saída antes de concluir.
  - Arquivos no worktree: `PLAN.md`, `progress.md`, `verification.md`.
- **+ Novo plano / Delegar:** escopo, item e quem escreve. O plano entra como "escrevendo", depois "aprovar" (faixa laranja: nada roda antes da sua aprovação), depois "executando".

### 8.5 Artefatos

Página geral com todos os artefatos do projeto (miniatura, tipo, quem editou, feature ligada), filtros por tipo e **+ Novo artefato**, que permite escolher o tipo ou pedir a um agente ("@Iara desenhe o fluxo de reembolso em Mermaid").

| Tipo | Como funciona |
|---|---|
| **Whiteboard** | Quadro livre com formas, setas, notas e cursores de agentes. Gera cards a partir do desenho. |
| **Mermaid** | Código e diagrama lado a lado (visões Código, Lado a lado, Diagrama). Sequência, fluxo, estados e ER. Agentes sugerem mudanças aplicáveis. |
| **PlantUML** | Classes, componentes, implantação e atividade, renderizado localmente. Pode ser gerado a partir do código. |
| **Documento** | Spec, runbook ou relatório com sumário, checklists, diagramas incorporados, sugestões de agentes (aceitar ou recusar) e comentários. Gera cards e testes de aceitação. |
| **Casos de uso** | Atores, pré e pós-condições, fluxo principal, fluxos alternativos e regras de negócio, com diagrama, rastreabilidade para cards e testes gerados. |
| **Anotações** | Captura rápida com tags e @agente, checklists, áudio transcrito, pesquisas de agentes. Uma nota pode virar card ou ADR. |
| **Decisão (ADR)** | Contexto, tabela de opções, decisão, ganhos e custos, linha do tempo de aprovação. Os agentes consultam ADRs aceitas antes de propor mudanças de arquitetura. |

Todos ficam salvos no repositório e podem ser lidos pelos agentes via `get_feature_context`.

### 8.6 Memória e grafo

- Fatos com autor, uso ("usado em 3 tarefas") e origem (conversa, tarefa ou artefato), que você aprova, edita ou esquece.
- Grafo de código (CodeGraph) para otimizações: quais arquivos se tocam, conflitos entre itens, onde uma mudança tem impacto.
- Memórias novas criadas por agentes chegam como decisão na barra.

**Artefatos na memória e no grafo**

- Os artefatos (8.5) também alimentam a memória e o grafo, porque descrevem o que o código deveria fazer.
- **Memória:** agentes extraem fatos de artefatos (ex.: os estados do pedido Pix a partir do diagrama Mermaid). O fato leva um link para o artefato de origem e aparece na aba "De artefatos". Fatos de ADRs aceitas valem como decisão.
- **Grafo:** artefatos são nós próprios, ligados aos arquivos que descrevem por arestas "descreve". O contexto de uma tarefa inclui os artefatos ligados ao caminho dela, não só os arquivos.
- **Desatualização:** quando um commit muda um arquivo ligado e o artefato não acompanha (ex.: um estado `estornado` novo que falta no diagrama), o grafo marca o artefato como desatualizado e sugere pedir a atualização a um agente, em Otimizações e como momento na barra de ação.
- **Ligações:** nascem quando um agente cria ou edita um artefato a partir de uma tarefa, quando uma forma do whiteboard é ligada a um arquivo, ou à mão. Ficam em `.vokia/artifacts/links.json`, versionado com o código.

### 8.7 Conversas e filas

É o lugar das conversas com os agentes. Cada mensagem de agente tem "+ virar pedido". A antiga tela "Thread e aprovações" saiu; o que ela tinha está aqui e no painel da barra.

- **Esquerda:** canais (#loja-online, #revisão, #pesquisa) e um item por agente, com estado, o que está fazendo e o tamanho da fila.
- **Centro:** conversa do agente com mensagens, ferramentas usadas, arquivos editados, pesquisas no navegador, **blocos de terminal** (comando, ok/falhou, duração, saída; os que falharam começam abertos), pedidos de aprovação dentro da conversa e "está editando…". Filtros Tudo, Conversa e Terminal.
- **Pedidos e fila:** pedidos rápidos com dois modos:
  - **Pôr na fila:** o pedido entra no fim, e o agente responde em qual posição.
  - **Interromper:** pausa a tarefa atual, que volta para a fila.
- **Direita:** fila do agente (Agora, com progresso, e próximos pedidos, com quem pediu; dá para subir ou tirar da fila), números do dia e link para a conversa completa.
- **Proposta em aberto** (prancheta "Proposta · Conversas unificada"): navegador do agente com "Assumir navegador" e escopo "este projeto / todos".

### 8.8 Agentes, roles e camadas

- Criar e configurar agentes: nome, foto ou avatar, role, CLI e modelo, skills, permissões.
- Roles com instruções, e os botões **Enhance** (melhorar a role com IA) e **Search** (buscar roles e templates prontos).
- Camadas de atuação e mini times (ex.: um implementador chama um revisor).

### 8.9 Estatísticas

- Por agente e por time: tarefas concluídas, aprovadas de primeira, devoluções, tempo médio, comandos, arquivos, tendência por dia, em 7, 14 ou 30 dias. Exemplo nos dados das telas: 46 tarefas em 14 dias, 78% aprovadas de primeira, 23 min por tarefa, 31 aprovações pedidas.
- **Micro-tarefas:** quanto de uso foi poupado com micro-tarefas em modelos leves e quantas precisaram subir de modelo.
- **Uso do plano:** por plano (Claude Code · Max, Codex · ChatGPT Plus), a janela de 5 h atual com o estado (com folga, perto do limite, pausado), as últimas janelas com o pico de cada uma, a semana dia a dia, quem consumiu nesta janela, pausas por limite em 14 dias, quanto o modo econômico poupou e quanto o grafo cortou de leitura. "Ajustar limites" leva a Configurações.
- Os componentes de limite (trilho e barra) ficam numa prancheta ao lado, "Limites de uso · componentes", e não nesta página.

### 8.10 Skills e superpowers

Biblioteca de skills (TDD, systematic-debugging, verification-before-completion, brainstorm, write-plan, execute-plan, skills do projeto). Mostra quem usa cada uma e permite pedir uma skill nova a um agente.

### 8.11 Diff e revisão

- **Agrupar por** feature, micro-tarefa, branch ou worktree. A lista da esquerda mostra cada grupo com o tamanho da mudança e o estado (em revisão, aprovado, devolvido, aguardando, agente trabalhando). Ex.: Pagamento via Pix (`vokia/checkout-pix → main`, 4 micro-tarefas), PIX-3, `vokia/cartao-parcelado`, worktree `checkout-pix`.
- **Arquivos e diff** do grupo, com comentários do revisor na linha ("Pedir ao Davi para corrigir" ou "Virar pedido").
- **Revisão:** quem revisa (o revisor do time, a líder para arquitetura e lotes, ou um agente de back-end) e quando: a cada micro-tarefa (automático ao concluir), ao fim da feature, antes do PR ou só quando você pedir. Mostra o parecer do revisor e o número da devolução ("1 de 2").
- **Verificação:** testes, typecheck e Playwright com o resultado, e avisos do grafo ("`payments/pix.ts` também está aberto em PAY-12; faça o merge desta feature primeiro").
- **Como prosseguir:** devolver ao autor com os comentários, aceitar e seguir (libera a próxima task ou o próximo lote), abrir PR (`gh pr create` com resumo e parecer), fazer merge (bloqueado enquanto houver problema aberto ou faltar revisão) ou descartar o worktree (mantém a branch). As opções que não valem para o estado atual aparecem desativadas, com o motivo.

### 8.12 Configurações

Valem para todos os projetos; regras de um projeto ou de um agente ficam nas páginas deles e têm prioridade. São **sub-páginas**, com navegação à esquerda e busca, não uma página só rolante:

| Sub-página | O que tem |
|---|---|
| **Geral** | Tema, idioma, densidade, abrir ao iniciar o computador, atalhos (⌘K, @, Espaço) e "Rever o onboarding". |
| **Agentes e conexões** | Claude Code, Codex, GitHub CLI e Git com versão, conta e "Testar"; "Verificar de novo"; modelo padrão por camada (Liderança, Interface e Serviços, Qualidade, Pesquisa) e para micro-tarefas (Haiku por padrão). A Vokia nunca lê nem guarda tokens. |
| **Execução** | Agentes ao mesmo tempo, devoluções antes de chamar você (1ª: a líder detalha o roteiro e o mesmo modelo tenta de novo · 2ª: sobe o modelo · depois: você), **Precisa do meu ok** (classificação do pedido, proposta da feature, a cada lote, merge e push), tempo máximo por tarefa, revisar a cada lote, exigir testes antes de concluir; pasta dos worktrees, prefixo das branches e limpeza após o merge. |
| **Permissões** | O que pede aprovação (instalar dependências, push e PR, internet, fora do worktree, apagar arquivos) e comandos sempre permitidos ou sempre bloqueados. |
| **Barra de ação e avisos** | Modo da barra, fora do app (bandeja, barra de tarefas ou lateral), chama na barra, fluxos em etapas, momentos, tempo do balão, notificações do sistema e não perturbe. |
| **Memória e grafo** | Confirmar fatos, extrair fatos de artefatos, aprender preferências, atualizar o grafo a cada commit, avisar artefato desatualizado, onde fica (`.vokia/memory/`, `.vokia/artifacts/links.json`) e reindexar. |
| **Uso e limites** | Uso atual de cada plano, pausar acima de X%, avisar acima de Y%, modo econômico e link para Estatísticas. |
| **Atualizações** | Versão, "Verificar agora", canal (estável ou beta), buscar ao abrir, instalar ao fechar (nunca com o time trabalhando) e notas da versão. |
| **Privacidade e dados** | Telemetria e relatórios de erro, dados em `~/.vokia`, exportar ou importar configurações e apagar dados do app. |

### 8.13 Onboarding e novo projeto

**Onboarding** (primeira abertura): seis telas limpas, uma coluna, uma pergunta por tela, com a barra de ação mostrando as etapas e "Pular e ir para o fim →" em todas.

1. **Boas-vindas:** o mascote, uma frase e a escolha de tema.
2. **Conexões:** Claude Code e Codex com o estado (logado, falta entrar, esperando o navegador). Dá para continuar só com o Claude.
3. **Projeto:** pasta local ou repositório do GitHub, começando de uma branch, de uma nova branch ou de um pull request.
4. **Time:** um modelo de time, ou começar sem time.
5. **Pedido:** o primeiro pedido para a líder, com sugestões ("Mapear o código", "Resolver issues").
6. **Pronto:** a líder já está lendo o projeto, três dicas da barra e "Abrir o mapa do time". O que foi pulado fica em Configurações.

**Novo projeto** (botão "+ Novo projeto"), em três etapas na barra:

1. **Origem:** pasta local ou GitHub (via `gh`, que nunca guarda o token), branch existente, nova branch ou PR, pasta do clone, nome, símbolo e descrição (vira o primeiro fato da memória). A prévia mostra o cartão em Projetos e o que a Vokia cria (`.vokia/` e `worktrees/`).
2. **Time:** reaproveitar um time de outro projeto (compartilhar os mesmos agentes ou copiar como time novo), usar um modelo ou começar vazio; escolher a orquestração (líder distribui, fila por camada, ou você atribui). A memória de cada projeto continua separada.
3. **Pedido:** o pedido para a líder, sugestões a partir do repositório e das issues abertas, contexto (memória de outro projeto, um plano, arquivos) e quando começar (planejar e esperar ou começar já).

---

## 9. Fluxos ponta a ponta

### 9.1 Do pedido ao merge

1. Você pede por "+ Novo pedido", "Nova feature" ou pelo campo da barra: "Quero frete grátis por região".
2. **A líder classifica em ~3 s**, num modelo leve (Haiku), e a sugestão aparece num balão acima da barra, com um clique para confirmar ou trocar:
   - "Parece pequeno · vai para o Davi": **Mandar** ou **É uma feature**. Confirmado, entra na fila do agente, sem card, e termina num commit ou PR ("Mandado para o Davi · Desfazer").
   - "Parece uma feature": **Começar triagem** ou **É pequeno**. Segue abaixo.
   - Atalhos pulam a classificação: `@agente` manda direto; `/feature` vai direto para a triagem; ⌘↵ abre o pedido detalhado. Por padrão a classificação sempre pede confirmação (configurável).
3. O card pai aparece no Backlog com "triagem", e a barra avisa que a líder começou.
4. A líder (sempre a do time) lê memória, grafo e artefatos e faz as perguntas que faltarem, como decisões.
5. Ela desdobra a feature em micro-tarefas com agente e modelo, e manda a proposta. A barra mostra "desdobrado em N micro-tarefas · Aprovar".
6. Você aprova. As micro-tarefas vão para Ready e a líder orquestra o mini time conforme as regras e o limite de paralelismo. A branch e o worktree nascem quando a primeira micro-tarefa entra em In progress.
7. Comandos sensíveis viram decisões de permissão.
8. Ao concluir uma micro-tarefa, a seta "tarefa concluída → pedir revisão" chama o revisor, e o card vai para In review.
9. Se a revisão reprova, a seta "devolver" manda de volta: na 1ª vez a líder **detalha o roteiro** com o que faltou (exemplo, passo ou teste novo) e o mesmo modelo tenta de novo; só na 2ª ela sobe o modelo; passado o limite (Configurações › Execução, igual em todas as setas), o fluxo chama você. O que faltou vira fato na memória (aba "De devoluções"), e os próximos roteiros parecidos já nascem com isso ("Já nasce com · aprendido da memória").
10. O card pai mostra o progresso ("4 micro-tarefas · 3 feitas"). Na entrega: verificação com evidências, Diff e revisão, PR e merge com você. Tudo vai para Done, e a feature ganha um replay (8.3.1).
11. Se um plano de uso chega perto do limite, a barra avisa em 75% e o time pausa em 90%, retomando quando a janela renova.

### 9.2 Decisões

- Toda decisão entra no painel da barra de ação, de todos os projetos, por tipo: permissão, pergunta, escolha, revisão, memória.
- Decisões de um clique podem aparecer em linha na barra compacta.
- A mesma decisão aparece também dentro da conversa do agente que pediu. O botão "decisões" da barra abre o painel (⌘J).
- Perguntas e escolhas sempre trazem "Outro…" com campo livre.
- Agentes parados esperando você ficam em laranja em todas as telas.

### 9.3 Fila e interrupção

- Pedidos para um agente ocupado entram na fila dele.
- "Interromper" pausa a tarefa atual, salva o progresso no worktree e coloca a tarefa de volta na fila.
- A ordem da fila pode ser mudada por você, e a orquestração pode reordenar conforme as regras.

### 9.4 Delegar um plano

- De Planos, do Backlog (card, modal ou "Triar pendentes") ou do componente Plano do item: escolher escopo, item e agente.
- O agente usa brainstorm e write-plan, lê memória e ADRs e entrega uma proposta.
- Nada executa antes da sua aprovação.

---

## 10. Sistema visual

### 10.1 Direção atual

- Duas direções no canvas:
  - **roxo sutil** (telas atuais);
  - **traço à mão + shadcn, minimalista** (tela Design). Componentes precisos de shadcn, com traço estilo Excalidraw só onde se pensa: canvas, notas e anotações.
- Tokens claro e escuro: fundo, superfícies, linha, tinta, texto suave, violeta (ativo), verde (trabalhando), laranja (precisa de você), vermelho (erro).
- Tipografia: Geist (interface), Geist Mono (código), Caveat (anotações à mão).
- Uma ação principal por tela (botão escuro). Violeta para aprovar algo de um agente.

### 10.2 Identidades visuais

- Biblioteca com 83 conceitos originais para agentes, projetos, documentos, equipes, fluxos e recursos.
- **Famílias de agentes:** Gotas, Cristais, Autômatos, Máscaras, Espíritos, Formas, Elementos, Objetos vivos, Pixel e low-poly, Minerais e joias.
- **Projetos:** emblemas (Ágora, Meridia, Lyra-6, Limiar, Dédalo, Forja, Raiz, Halo), com três tipos de moldura.
- **Documentos:** uma mesma folha com um desenho por tipo.
- **Estados por forma:**
  - executando: anel tracejado e ▶;
  - esperando: pílula com três pontos;
  - concluído: selo com ✓;
  - falha: losango com ✕;
  - pausado: esmaecido e ‖.
- Tudo desenhado em 64 px e testado em 16 px para listas, árvores, chat e painéis.

### 10.3 Decisões de interface

**Tomadas:**

- A barra de ação fica fixa no centro inferior de todas as telas e guia os fluxos em etapas.
- Saíram as telas "Chat e kanban" e "Thread e aprovações", e os itens "Chat e kanban" e "Aprovações" do trilho. Pedidos vão pela barra, o kanban é o Backlog, e as aprovações ficam no painel de decisões e nas Conversas.
- O kanban do backlog usa as colunas Backlog, Ready, In progress, In review e Done.
- Todo pedido entra por um compositor único. A líder do time sempre faz a triagem; features viram card pai e se desdobram em micro-tarefas com modelos mais baratos; pedidos pequenos vão direto para um agente, sem card, e o rastro fica no git.
- Os limites de uso aparecem no avatar do trilho, na barra só nos limiares, e com histórico em Estatísticas.
- Configurações em sub-páginas.
- Cada grupo de componentes tem uma prancheta ao lado da página que os usa.
- Toda pergunta ou múltipla escolha tem "Outro…".
- O modelo de cada micro-tarefa aparece só no modal, não no card.
- Fora do app, a barra pode ficar na bandeja, na barra de tarefas ou na lateral da tela.

**Em aberto:**

- **Navegador do agente nas Conversas:** a proposta "Conversas unificada" (Assumir navegador, escopo este projeto / todos).
- **Aplicar a direção minimalista** às telas restantes.
- **Escolher as famílias de avatares** que substituem os retratos atuais.

---

## 11. Segurança e permissões

- O app nunca toca em credenciais. Login e troca de conta acontecem na própria CLI.
- Permissões por ferramenta e por projeto (`--allowedTools`, `--permission-mode`). Pedidos fora do permitido viram decisões.
- "Sempre neste projeto" grava a permissão no projeto, de forma visível e reversível em Configurações.
- Merge e push sempre passam por você, salvo se a orquestração estiver configurada para "nunca parar" (merge automático), o que aparece de forma explícita no fluxo.
- Telemetria desligada por padrão. Dados locais.
- A busca de atualização só fala com o GitHub Releases do repositório oficial e não envia dados do usuário. O updater do Tauri confere a assinatura de cada download com a chave pública embutida no app, antes de instalar.

---

## 12. Roadmap

| Fase | Entrega |
|---|---|
| 0 · Base | App Tauri, detecção das CLIs, um agente rodando em headless com saída em stream-json, SQLite, executável para Windows, macOS e Linux, busca de atualização no GitHub Releases ao iniciar, GitHub Action de release por tag |
| 1 · Um projeto, um time | Onboarding e novo projeto, Projetos, worktree por feature, barra de ação com pedidos e proposta de cards, kanban do backlog, Conversas com terminal |
| 2 · Orquestração | Camadas, roles, servidor MCP (delegate, report, ask_human), regras das setas, limites, decisões na barra |
| 3 · Planos e artefatos | Planos por feature/task, delegação de plano, Backlog e planos, artefatos (Mermaid, PlantUML, Documento, ADR, Casos de uso, Anotações, Whiteboard) |
| 4 · Qualidade | Diff e revisão por feature, task, branch e worktree, verificação com evidências, memória e grafo, Estatísticas, limites de uso com aviso e pausa |
| 5 · Escala | Vários projetos em paralelo, filas e interrupção, Skills, templates de times |
| 6 · Acabamento | Sistema visual final, identidades de agentes, temas, acessibilidade, onboarding |

## 13. Riscos

- **Termos de uso das CLIs:** o app executa as CLIs oficiais sem modificar e sem intermediar tokens. Isso precisa ser mantido a cada atualização.
- **Conflitos no mesmo worktree:** mitigado pelo grafo de código e pela ordem dos lotes.
- **Custo e limites de uso:** muitos agentes em paralelo consomem rápido a janela de 5 h. Mitigação: medidor no trilho, aviso em 75%, pausa em 90%, modo econômico, troca para o outro CLI, limite de paralelismo e histórico em Estatísticas. A leitura do uso depende do que cada CLI expõe e precisa ser validada.
- **Atualização no macOS:** sem assinatura e notarização da Apple, a atualização automática não funciona. Mitigação: aviso com link até ter a conta de desenvolvedor.
- **Release quebrada:** como todos atualizam ao abrir, uma versão com erro chega a todos. Mitigação: GitHub Action com testes antes de publicar, e releases marcadas como pré-lançamento para testar antes.
- **Excesso de decisões:** a barra precisa agrupar e priorizar, com "Sempre neste projeto" e decisões em linha para pedidos simples.

## 14. Telas no canvas

| Grupo | Telas |
|---|---|
| Visão geral | Projetos, Mapa do time, Mapa do time · Orquestração |
| Planejar | Backlog e planos, Backlog e planos · compacto, Backlog · componentes, Pedidos · componentes, Replay · Login com Google, Planos, Memória e grafo |
| Fluxo do pedido | 1 escrever na barra, 2 Iara vê o tamanho, 3a sugestão: pequeno, 4a mandado ao agente, 4a′ agente devolve como feature, 3b sugestão: feature, 4b triagem, 5 perguntas da líder, 6 proposta de micro-tarefas, 7 aprovado |
| Agentes e conversas | Agentes, roles e camadas, Estatísticas, Limites de uso · componentes, Skills e superpowers, Conversas e filas, Proposta · Conversas unificada |
| Revisão e configuração | Diff e revisão, Configurações (9 sub-páginas) |
| Primeiros passos | Onboarding (Boas-vindas, Conexões, Projeto, Time, Pedido, Pronto), Novo projeto (Origem, Time, Pedido) |
| Barra fora do app | Ícone na bandeja, Na barra de tarefas, Lateral da tela |
| Artefatos | Artefatos, Whiteboard, Mermaid, PlantUML, Documento, Casos de uso, Anotações, Decisão (ADR) |
| Componentes | Trilho de ícones, Barra de ação, Retrato, Plano do item, Card do backlog, Modal do item, Novo pedido |
| Sistema visual | Design · nova direção, Barra de ação · variações, Identidades visuais, Marca e mascote |
