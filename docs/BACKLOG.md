# Backlog · Vokia

Requisitos derivados do protótipo [Design.html](prototipo/Design.html) e [Fora do app.html](prototipo/Fora%20do%20app.html) e do diagrama de estados [barra-de-acao.puml](diagramas/barra-de-acao.puml). Levantamento de 2026-10-26. Cada requisito tem um documento com objetivo, comportamento, estados, critérios de aceite, referências, dependências e pendências.

## Estado de implementação

Conferido no código em 2026-10-28, em `desktop/src`, para os itens ligados às etapas concluídas do Plano Roadmap (1, C1, F1, 2, 3, 4, 5, 6, 6R, 7, 7B, 7C, 7D, 8, 8R, 9, 10, 11 e 30). Os demais requisitos não foram conferidos aqui.

| ID | Estado | Falta | Documento |
| --- | --- | --- | --- |
| 001 | Parcial | Parte "/" (Etapa 14) e painel de decisões (Etapa 17) | [001](backlog/001.md) |
| 002 | Parcial | Contador de decisões na barra compacta (Etapa 17) | [002](backlog/002.md) |
| 003 | Parcial | "Precisa de você" (Etapa 17) e progresso no anel | [003](backlog/003.md) |
| 004 | Parcial | Chama no balão de limite de plano (Etapa 21) | [004](backlog/004.md) |
| 005 | Parcial | ⌥2 não abre o compositor (Etapa 15) | [005](backlog/005.md) |
| 006 | Parcial | Placeholder por página (Projetos e Chat) | [006](backlog/006.md) |
| 007 | ✅ Concluído | Lista mostra os agentes do time, não os do quadro | [007](backlog/007.md) |
| 022 | ✅ Concluído | Só a página Conversas tem tela (demais: Etapas 31 a 44) | [022](backlog/022.md) |
| 023 | Parcial | Anel e linha do GPT (sem fonte real do Codex) | [023](backlog/023.md) |
| 027 | ✅ Concluído | — | [027](backlog/027.md) |
| 028 | ✅ Concluído | Fonte Caveat e escala "à mão · 20" | [028](backlog/028.md) |
| 029 | Parcial | Formas à mão do canvas; abas e dados (Etapa 2B, em revisão) | [029](backlog/029.md) |
| 030 | Parcial | Área livre do símbolo (`VokiaSignature.vue:13`) a confirmar | [030](backlog/030.md) |
| 031 | ✅ Concluído | — | [031](backlog/031.md) |
| 032 | ✅ Concluído | 5 famílias extras do quadro (pendentes na Etapa 5) | [032](backlog/032.md) |
| 033 | Parcial | Splash sem "e Codex"; tela de vazio sem features | [033](backlog/033.md) |

## A1 · Barra: estrutura e estado

| ID | Nome | O que faz | Documento |
| --- | --- | --- | --- |
| 001 | Anatomia e posição da barra de ação | Fixa a barra na parte inferior de todas as telas, com quatro partes na mesma ordem, em uma linha. | [001](backlog/001.md) |
| 002 | Barra compacta e expandida | Mostra a barra compacta por padrão e a expande no lugar com o mouse ou o foco no campo. | [002](backlog/002.md) |
| 003 | Estado do time e prioridade | Mostra no anel e no status um estado do time por vez, o de maior prioridade. | [003](backlog/003.md) |
| 004 | Mascote na barra | Usa a chama no lugar do anel só em mensagens gerais e no balão de limite de plano. | [004](backlog/004.md) |
| 005 | Menu de ações por página | Abre pelo ☰ um menu com as ações da página atual, disparáveis pelos atalhos ⌥n. | [005](backlog/005.md) |

## A2 · Barra: pedido

| ID | Nome | O que faz | Documento |
| --- | --- | --- | --- |
| 006 | Campo de pedido | Permite pedir algo ao projeto direto da barra expandida, com placeholder que indica o que digitar. | [006](backlog/006.md) |
| 007 | Menção com @ | Lista os agentes do time ao digitar @ no campo de pedido, para escolher quem recebe. | [007](backlog/007.md) |
| 008 | Comandos com / | Lista os comandos do projeto ao digitar / no campo de pedido, para escolher um. | [008](backlog/008.md) |
| 009 | Compositor de novo pedido | Abre um modal para descrever o pedido, escolher o tamanho e o agente de destino. | [009](backlog/009.md) |
| 010 | Triagem do pedido no balão | Mostra num balão acima da barra a etapa do pedido, da escrita até a aprovação. | [010](backlog/010.md) |

## A3 · Barra: decisões e acompanhamento

| ID | Nome | O que faz | Documento |
| --- | --- | --- | --- |
| 011 | Painel de decisões | Reúne num painel da barra as decisões que os agentes esperam, para responder sem sair da tela. | [011](backlog/011.md) |
| 012 | Decisão em linha | Mostra na própria linha da barra uma decisão de um clique, sem abrir o painel. | [012](backlog/012.md) |
| 013 | Painel Acontecendo agora | Mostra o que cada agente faz: tarefa, etapas, últimas ações, fila e números do dia. | [013](backlog/013.md) |

## A4 · Barra: avisos e fluxos

| ID | Nome | O que faz | Documento |
| --- | --- | --- | --- |
| 014 | Balão de aviso: regras gerais | Define as regras de todo balão: um por vez, some sozinho, origem visível, pode ser calado. | [014](backlog/014.md) |
| 015 | Momentos do time | Avisa em balão quando a chama percebe algo que vale guardar, como fato, decisão ou conflito. | [015](backlog/015.md) |
| 016 | Limites do plano na barra | Mostra o uso de 5 h do plano só ao cruzar limiar: aviso em 75%, pausa em 90%, renovação. | [016](backlog/016.md) |
| 017 | Fluxo em etapas na barra | Mostra as etapas de um fluxo guiado no lugar do ☰ e do botão de decisões. | [017](backlog/017.md) |

## B · Fora do app

| ID | Nome | O que faz | Documento |
| --- | --- | --- | --- |
| 018 | Barra fora do app: ícone na bandeja | Mostra pelo ícone da bandeja o estado dos projetos e agentes, decisões de comando e limites por plano. | [018](backlog/018.md) |
| 019 | Barra fora do app: widget na barra de tarefas | Mostra na barra de tarefas o resumo dos agentes e um contador; o clique alterna o aviso. | [019](backlog/019.md) |
| 020 | Barra fora do app: lateral da tela | Prende a barra à borda direita da tela, com resumo dos agentes e campo de pedido. | [020](backlog/020.md) |
| 021 | Notificação fora do app | Avisa com a Vokia fora de foco que uma proposta ficou pronta; só comandos são aprovados fora do app. | [021](backlog/021.md) |

## C · Navegação e componentes

| ID | Nome | O que faz | Documento |
| --- | --- | --- | --- |
| 022 | Trilho de ícones | Dá acesso às páginas do workspace e às ferramentas do projeto, recolhido ou expandido. | [022](backlog/022.md) |
| 023 | Uso dos planos no trilho | Mostra no trilho quanto da janela de 5 h de cada plano foi usado, em anéis e painel. | [023](backlog/023.md) |
| 024 | Card do backlog | Representa um item do backlog ou tarefa do kanban com identificação, estado, progresso e próxima ação. | [024](backlog/024.md) |
| 025 | Modal do item do backlog | Abre um item do backlog com triagem, etiquetas, micro-tarefas, critérios, atividade, plano e vínculos. | [025](backlog/025.md) |
| 026 | Plano do item | Vincula, pede, revisa e desvincula o plano de um item; agentes só começam com plano. | [026](backlog/026.md) |
| 027 | Retrato de agente | Identifica cada agente com uma ilustração quadrada, igual em todos os tamanhos. | [027](backlog/027.md) |

## D · Sistema visual e marca

| ID | Nome | O que faz | Documento |
| --- | --- | --- | --- |
| 028 | Tokens de design e tema claro/escuro | Define tokens de cor, tipografia, medidas e ícones nos temas claro e escuro, e os três princípios visuais. | [028](backlog/028.md) |
| 029 | Componentes base de interface | Define os componentes base reutilizados: botões, campos, selos, cartões, código, comando e avisos de retorno. | [029](backlog/029.md) |
| 030 | Logo e ícone do app | Define a logo da Vokia e as variações do ícone no executável, no dock e na bandeja. | [030](backlog/030.md) |
| 031 | Mascote da Vokia | Define o desenho do mascote, a chama com rosto, braços e pernas, com poses e estados visuais. | [031](backlog/031.md) |
| 032 | Biblioteca de identidades visuais | Define avatares, emblemas e símbolos de agentes, projetos, documentos e recursos, com marcas de estado além da cor. | [032](backlog/032.md) |
| 033 | Telas de sistema: splash, atualização, vazio e CLI ausente | Define as quatro telas de sistema: splash, atualização pronta, vazio sem features e CLI não encontrada. | [033](backlog/033.md) |

## E · Conversa do time

| ID | Nome | O que faz | Documento |
| --- | --- | --- | --- |
| 034 | Menção @todos e reações nas mensagens | Envia um pedido a todos os agentes de uma vez com @todos e permite reagir com emoji às mensagens, sem poluir o feed. | [034](backlog/034.md) |

## F · Agentes e time

| ID | Nome | O que faz | Documento |
| --- | --- | --- | --- |
| 035 | Criação de agentes pela interface | Permite criar agentes pela interface, ao lado da Iara e do Davi, com nome, papel, retrato, CLI, modelo, skills e permissões. | [035](backlog/035.md) |

## Pendências do diagrama

Sete pendências foram levantadas no diagrama; a 7 foi removida porque o modo "desligada" não existe. As decisões já aceitas e as pendências abertas (PEN) estão em [PENDENCIAS.md](PENDENCIAS.md). Os requisitos abaixo citam cada ponto como `Pendência do diagrama (n)`.

| Nº | Pendência | Pendências abertas (PEN) | Aparece em |
| --- | --- | --- | --- |
| 1 | Critério de decisão de um clique (decidido; refletido na nota (e) do diagrama) | [PEN-015](pendencias/PEN-015.md) | [012](backlog/012.md) |
| 2 | Condição de "Esperando você" e fim do fluxo em etapas (decidido em Decisões aceitas) | — | [017](backlog/017.md) |
| 3 | Saída de Concluído e posição de Em paralelo e Pensando na ordem de prioridade | [PEN-001](pendencias/PEN-001.md), [PEN-002](pendencias/PEN-002.md), [PEN-012](pendencias/PEN-012.md) | [003](backlog/003.md) |
| 4 | Critério que separa sugestão pequena de feature e entrada de momentos com o painel aberto | [PEN-010](pendencias/PEN-010.md) | [009](backlog/009.md), [010](backlog/010.md), [014](backlog/014.md) |
| 5 | avisoOff, fechamento dos botões de balão, painel ao entrar em fluxo e efeito de "Permitir" | [PEN-003](pendencias/PEN-003.md), [PEN-013](pendencias/PEN-013.md) | [010](backlog/010.md), [011](backlog/011.md), [014](backlog/014.md), [017](backlog/017.md), [018](backlog/018.md) |
| 6 | Divergências protótipo x regras: cor do Dock, texto "4 trabalhando", "um agente", mascote, momentos, decisão em linha | [PEN-011](pendencias/PEN-011.md), [PEN-014](pendencias/PEN-014.md), [PEN-015](pendencias/PEN-015.md) | [003](backlog/003.md), [004](backlog/004.md), [012](backlog/012.md), [015](backlog/015.md), [016](backlog/016.md), [031](backlog/031.md) |
| 7 | Removida: o modo "desligada" não existe | — | — |
