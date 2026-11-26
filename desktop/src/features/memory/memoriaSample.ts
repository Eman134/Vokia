export interface MemoryFact {
  text: string
  // null marca a preferência do usuário, que usa a inicial em vez do retrato.
  who: string | null
  author: string
  usage: string
  suggested: boolean
}

export const MEMORY_FACTS: readonly MemoryFact[] = [
  {
    text: 'O webhook do PSP manda o txid em maiúsculas; o front normaliza.',
    who: 'leo',
    author: 'Léo',
    usage: 'da conversa Davi e Nina · usado em 3 tarefas',
    suggested: false,
  },
  {
    text: 'Roteiros que buscam pedido por um id do PSP já incluem o passo de normalizar o id e um exemplo do payload real.',
    who: 'ember',
    author: 'Ember',
    usage: 'da devolução de PIX-4, que o Haiku resolveu com o roteiro v2 · aplicado em PAY-14.2',
    suggested: false,
  },
  {
    text: 'QR Code do Pix expira em 15 minutos.',
    who: 'ember',
    author: 'Ember',
    usage: 'decisão aprovada por você · usado em 2 tarefas · ADR-004',
    suggested: false,
  },
  {
    text: 'Nunca instalar dependência sem pedir aprovação.',
    who: null,
    author: 'sua preferência',
    usage: 'vale para todos os agentes',
    suggested: false,
  },
  {
    text: 'Testes de pagamento rodam com PSP_SANDBOX=1; sem isso a suíte chama a API real.',
    who: 'nina',
    author: 'Nina',
    usage: 'aprendido depois de um teste falhar · usado em 5 tarefas',
    suggested: false,
  },
  {
    text: 'O pedido Pix passa por pendente → pago → expirado; só pago libera a nota.',
    who: 'davi',
    author: 'Davi',
    usage: 'extraído do diagrama · usado em 4 tarefas · Fluxo do pagamento Pix',
    suggested: false,
  },
  {
    text: 'O checkout precisa funcionar com scripts de terceiros bloqueados.',
    who: 'teo',
    author: 'Téo sugeriu',
    usage: 'espera você confirmar',
    suggested: true,
  },
]

export const MEMORY_NOTE =
  'Salva em .vokia/memory/ como Markdown, versionada com o código. Cada fato guarda a origem: conversa, tarefa ou artefato.'

export const GRAPH_DESCRIPTION = 'decisões, palavras-chave, memórias, cards, arquivos e artefatos conectados'

export const GRAPH_EMPTY_TITLE = 'Nenhum nó no grafo ainda'
