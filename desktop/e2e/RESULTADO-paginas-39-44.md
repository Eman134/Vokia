# Resultado · Etapas 39 (Configurações) e 44 (Artefatos)

**Veredito:** PASSOU. 53 de 53 checks. Nenhum erro de console nem exceção no roteiro inteiro.
**Produção:** não alterada (só arquivos novos em `desktop/e2e`).

## Como repetir

```
cd desktop && bash e2e/paginas-39-44-rodar.sh
```

O script sobe o Vite de teste (`e2e/vite.telas.config.ts`, mocks do Tauri) na porta 4190 e o Edge headless com CDP na 9346, roda `e2e/paginas-39-44.cdp.mjs` e para só os dois processos que iniciou. Não usa a 1420. Saída: `e2e/evidencias-paginas-39-44/` (`run.log`, `resultados.json`, capturas PNG).

## Formas de falha consideradas

- Configurações: tema mudando sem o seletor refletir, ou perdendo a escolha ao reabrir a página; controle desabilitado ainda recebendo clique; seção estourando 860 px; barra de ação cobrindo a última seção; rolagem horizontal.
- Artefatos: cartão abrindo tela quebrada/em branco; contagem diferente do número de cartões; grade quebrando a 1200 px; erro de console ao clicar; botão e filtro desabilitados reagindo ao clique; seletor "Que tipo de artefato?" aparecendo no estado inicial.

## Cenários e resultado (1440x900 e 1200x780, exceto tema)

| Página | Verificação | Resultado |
|---|---|---|
| Configurações | contexto "Workspace · dados de exemplo", título, subtítulo, busca desabilitada | PASSOU |
| Configurações | nove seções na ordem do protótipo | PASSOU |
| Configurações | navegação lateral: 9 entradas desabilitadas, só Geral ativa; indicador "Tudo conectado" | PASSOU |
| Configurações | 82 controles fora do Tema, todos desabilitados; controle positivo: Tema tem 3 botões habilitados | PASSOU |
| Configurações | cliques reais em alternador, Densidade "Compacta" e navegação lateral não mudam nada | PASSOU |
| Configurações | seção ≤ 860 px (823 a 1200 px), sem conteúdo cortado; última seção acima da barra (750 < 828 e 630 < 708) | PASSOU |
| Configurações | tema: Sistema inicial; Escuro/Claro/Sistema aplicam `data-theme` e marcam a opção; fundo muda (rgb 18,18,20 / 250,250,250); sair e voltar mantém Escuro | PASSOU |
| Artefatos | contexto "loja-online · dados de exemplo", título, subtítulo, "+ Novo artefato" desabilitado | PASSOU |
| Artefatos | seletor: Todos 9 ativo e as contagens do protótipo, todos desabilitados; "9 artefatos"; soma por tipo = 9 | PASSOU |
| Artefatos | 9 cartões com tipo (ícone), título e autor · tempo exatos do protótipo | PASSOU |
| Artefatos | clicar nos 9 cartões, no botão e num filtro: HTML, URL, item do trilho idênticos, sem diálogo, sem erro | PASSOU |
| Artefatos | grade sem sobreposição e dentro do contêiner: 5 colunas a 1440 px, 4 colunas a 1200 px (cartão mín. 251/258 px) | PASSOU |
| Ambas | sem rolagem horizontal (documento, corpo e coluna rolável) em 1440 e 1200 | PASSOU |
| Console | nenhuma exceção nem `console.error` | PASSOU |

## Lacunas

- Prova de mutação em produção não feita (o pedido proibiu alterar código de produção). Substituta: controle positivo no mesmo roteiro, que prova que o detector de controles habilitados enxerga o seletor de tema.
- O tema não é persistido entre recarregamentos da página (só em memória na store), conforme a nota ("Não fazer: persistir"). O "reabrir" testado é sair e voltar à página.
- Detalhe visual (cores, tipografia) não coberto por teste; ver capturas.
- Typecheck, build e lint não rodados nesta etapa (o pedido cobria só o E2E).
