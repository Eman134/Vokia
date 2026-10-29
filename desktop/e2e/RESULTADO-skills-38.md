# RESULTADO - Etapa 38 · Página Skills (rodada 2, atual)

Veredito: **PASSOU** (35 de 35 verificações)

Repetir: a partir de `desktop/`, `bash e2e/skills-memoria-rodar.sh` (Vite de teste na 4192 e Edge headless com CDP na 9348; só esses dois processos são parados no fim).
Arquivo de teste: `desktop/e2e/skills-memoria.cdp.mjs`. Evidências (capturas, `resultados.json`, `run.log`): `desktop/e2e/evidencias-skills-38-memoria-42/`.

Cobre cabeçalho, três cartões, matriz "Quem usa o quê", ações e marcadores desabilitados, rolagem da matriz a 820 px e leiaute a 1200 e 1440 px. Os detectores foram provados em "Mutação no DOM" (a produção não é alterada).

| Resultado | Cenário | Verificação |
|---|---|---|
| PASSOU | Skills 1440x900 | contexto "Workspace · 8 skills · dados de exemplo", título "Skills" e subtítulo do protótipo |
| PASSOU | Skills 1440x900 | ações "Criar da memória" e "Instalar skill" nessa ordem, ambas desabilitadas com aria-disabled |
| PASSOU | Skills 1440x900 | três cartões na ordem Superpowers, shadcn/ui, pix-loja-online |
| PASSOU | Skills 1440x900 | cartão Superpowers: origem, descrição, chips e frase "Ligada para" exatos |
| PASSOU | Skills 1440x900 | cartão shadcn/ui: origem, descrição, chips e frase "Ligada para" exatos |
| PASSOU | Skills 1440x900 | cartão pix-loja-online: origem, descrição, chips e frase "Ligada para" exatos |
| PASSOU | Skills 1440x900 | cartões dentro da grade, sem sobreposição, sem conteúdo cortado e com chips dentro do cartão (forma de falha: chips quebrando o cartão) |
| PASSOU | Skills 1440x900 | matriz "Quem usa o quê" presente com cabeçalho Skill + seis ocupações na ordem do protótipo |
| PASSOU | Skills 1440x900 | cada ocupação tem retrato com arte (forma de falha: retrato sem arte), tamanho 22 e rótulo do agente certo |
| PASSOU | Skills 1440x900 | sete linhas com o nome da skill e o marcador ligado/desligado de cada célula iguais ao protótipo |
| PASSOU | Skills 1440x900 | 42 marcadores (7x6), 21 ligados, todos aria-disabled="true" |
| PASSOU | Skills 1440x900 | marcadores ligados têm preenchimento e desligados não (distinção visual real) |
| PASSOU | Skills 1440x900 | painel da matriz tem overflow-x auto: tabela mais larga que o painel rola dentro dele em vez de estourar a página (forma de falha: matriz estourando sem rolagem) |
| PASSOU | Skills 1440x900 | sem rolagem horizontal no documento nem na coluna rolável da página |
| PASSOU | Skills 1440x900 | clicar nas duas ações, em marcadores da matriz e num cartão não muda a página (mesmo HTML, URL, trilho, sem diálogo) |
| PASSOU | Skills 1200x780 | contexto "Workspace · 8 skills · dados de exemplo", título "Skills" e subtítulo do protótipo |
| PASSOU | Skills 1200x780 | ações "Criar da memória" e "Instalar skill" nessa ordem, ambas desabilitadas com aria-disabled |
| PASSOU | Skills 1200x780 | três cartões na ordem Superpowers, shadcn/ui, pix-loja-online |
| PASSOU | Skills 1200x780 | cartão Superpowers: origem, descrição, chips e frase "Ligada para" exatos |
| PASSOU | Skills 1200x780 | cartão shadcn/ui: origem, descrição, chips e frase "Ligada para" exatos |
| PASSOU | Skills 1200x780 | cartão pix-loja-online: origem, descrição, chips e frase "Ligada para" exatos |
| PASSOU | Skills 1200x780 | cartões dentro da grade, sem sobreposição, sem conteúdo cortado e com chips dentro do cartão (forma de falha: chips quebrando o cartão) |
| PASSOU | Skills 1200x780 | matriz "Quem usa o quê" presente com cabeçalho Skill + seis ocupações na ordem do protótipo |
| PASSOU | Skills 1200x780 | cada ocupação tem retrato com arte (forma de falha: retrato sem arte), tamanho 22 e rótulo do agente certo |
| PASSOU | Skills 1200x780 | sete linhas com o nome da skill e o marcador ligado/desligado de cada célula iguais ao protótipo |
| PASSOU | Skills 1200x780 | 42 marcadores (7x6), 21 ligados, todos aria-disabled="true" |
| PASSOU | Skills 1200x780 | marcadores ligados têm preenchimento e desligados não (distinção visual real) |
| PASSOU | Skills 1200x780 | painel da matriz tem overflow-x auto: tabela mais larga que o painel rola dentro dele em vez de estourar a página (forma de falha: matriz estourando sem rolagem) |
| PASSOU | Skills 1200x780 | sem rolagem horizontal no documento nem na coluna rolável da página |
| PASSOU | Skills 1200x780 | clicar nas duas ações, em marcadores da matriz e num cartão não muda a página (mesmo HTML, URL, trilho, sem diálogo) |
| PASSOU | Skills 820x700 (estreito) | sem rolagem horizontal no documento nem na coluna rolável da página |
| PASSOU | Skills 820x700 (estreito) | estreito: matriz com rolagem horizontal própria (conteúdo maior que o painel) e cartões sem estourar |
| PASSOU | Skills 820x700 (estreito) | estreito: rolando a matriz até o fim a coluna Pesquisa fica visível |
| PASSOU | Mutação no DOM (detectores) | detector pega ação habilitada, célula alterada, retrato sem arte e matriz sem rolagem |
| PASSOU | Console | nenhuma exceção nem console.error durante todo o roteiro |
