export const PROJECTS_TITLE = 'Seus projetos'
export const PROJECTS_SUBTITLE =
  'Cada projeto tem um símbolo e o seu próprio mapa do time, backlog, conversas, artefatos e memória. Um agente pode trabalhar em mais de um.'
export const NEW_PROJECT_LABEL = '+ Novo projeto'
export const OPEN_LABEL = 'Abrir'
export const EDIT_LABEL = 'Editar'
export const SAVE_LABEL = 'Salvar'
export const EMPTY_PATH_TEXT = 'Sem pasta definida'

export const NEW_PROJECT_CONTEXT = 'Projetos › Novo projeto'
export const NEW_PROJECT_STEPS = ['Origem', 'Montar o time', 'Primeiro pedido'] as const
export const STEP_TITLES = ['Novo projeto', 'Montar o time', 'Primeiro pedido'] as const
export const STEP_SUBTITLES = [
  'De uma pasta do computador ou de um repositório do GitHub. Nos próximos passos você escolhe o time e faz o primeiro pedido.',
  'Reaproveite um time que já trabalha em outro projeto, comece de um modelo ou deixe para depois.',
  'Diga o que o time deve fazer primeiro. A líder transforma o pedido em uma feature com cards e você decide quando começa.',
] as const
export const PREVIEW_HEADING = 'Prévia em Projetos'
export const TEAM_NEXT_HEADING = 'Time no próximo passo'
export const CREATES_HEADING = 'O que a Vokia cria'
export const CREATES_ITEMS = [
  { name: '.vokia/', text: 'memória, plano e camadas do projeto' },
  { name: 'worktrees/', text: 'um por feature, fora da sua branch' },
] as const
export const CREATES_NOTE = 'Nada é enviado ao GitHub sem a sua aprovação.'
export const LINKED_REPO_LABEL = 'Vinculado ao GitHub'
export const CLI_NOTE = 'Claude Code e Codex logados'
export const START_FROM_LABEL = 'Começar de'
export const START_FROM_VALUE = 'Branch existente'
export const SAMPLE_TEAM_PREVIEW = ['Iara', 'Davi', 'Nina'] as const

export const SAMPLE_REUSE_MODES = ['Reaproveitar um time', 'Usar um modelo', 'Começar vazio'] as const
export const SAMPLE_TEAMS_HEADING = 'Times dos seus projetos'
export const SAMPLE_TEAMS = [
  { name: 'Time Web', from: 'loja-online', desc: 'liderança, implementação e revisão' },
  { name: 'Mobile', from: 'app-agenda', desc: 'app nativo, testes e pesquisa' },
  { name: 'Site', from: 'site-estudio', desc: 'design e front-end' },
] as const
export const SAMPLE_SHARE_HEADING = 'Como reaproveitar'
export const SAMPLE_SHARE_OPTIONS = [
  { label: 'Compartilhar os agentes', hint: 'os mesmos agentes trabalham nos dois projetos' },
  { label: 'Copiar como time novo', hint: 'cópias independentes, configurações iguais' },
] as const
export const SAMPLE_AGENTS_HEADING = 'Agentes'
export const SAMPLE_AGENTS_COUNT = '3 de 4 incluídos'
export const SAMPLE_AGENTS = [
  { id: 'iara', name: 'Iara', role: 'líder · planeja e divide', cli: 'claude', layer: 'Produto', load: 'ocupada em 2 projetos' },
  { id: 'davi', name: 'Davi', role: 'front-end', cli: 'claude', layer: 'Interface', load: 'em 2 projetos' },
  { id: 'nina', name: 'Nina', role: 'back-end e dados', cli: 'codex', layer: 'Serviços', load: 'livre' },
  { id: 'teo', name: 'Téo', role: 'revisão e testes', cli: 'claude', layer: 'Qualidade', load: 'em 2 projetos' },
] as const
export const SAMPLE_ORCH_HEADING = 'Orquestração'
export const SAMPLE_ORCH_OPTIONS = [
  { label: 'Líder distribui', hint: 'Iara divide em cards e passa adiante' },
  { label: 'Fila por camada', hint: 'cada agente puxa da sua camada' },
  { label: 'Livre', hint: 'você atribui cada card' },
] as const
export const SAMPLE_LAYERS_HEADING = 'Camadas'
export const SAMPLE_LAYERS = ['Produto', 'Interface', 'Serviços'] as const
export const SAMPLE_LAYERS_HINT = 'camadas copiadas de loja-online'
export const TEAM_FOOTER_HINT = 'dá para mudar o time a qualquer momento'
export const CONTINUE_TO_REQUEST_LABEL = 'Continuar para o pedido'
export const MAP_PREVIEW_HEADING = 'Prévia do mapa'
export const MAP_PREVIEW_NOTE = '3 agentes · líder distribui'
export const SHARED_HEADING = 'Agentes compartilhados'
export const SHARED_NOTE =
  'Os pedidos deste projeto entram na mesma fila de cada agente. A memória de cada projeto continua separada. O agente leva só o que aprendeu sobre o próprio jeito de trabalhar.'

export const BACK_LABEL = 'Voltar'
export const CANCEL_LABEL = 'Cancelar'
export const CHOOSE_LABEL = 'Escolher'
export const PICK_FOLDER_LABEL = 'Escolher pasta'
export const CREATE_WITHOUT_REQUEST_LABEL = 'Criar sem pedido'

export const REQUEST_TO_HEADING = 'O pedido vai para'
export const REQUEST_LEAD = 'Iara'
export const REQUEST_LEAD_NOTE = ', que lidera o time'
export const REQUEST_CREW_NOTE = 'Davi e Nina recebem os cards'
export const REQUEST_FIELD_LABEL = 'O que o time deve fazer primeiro?'
export const REQUEST_TEXT =
  'Criar a área do cliente: login por link mágico, lista de pedidos com status e segunda via das faturas. Reaproveitar a API de pedidos da loja-online.'
export const REQUEST_COMPOSER_HINT = '@ menciona · / comandos'
export const REQUEST_SUGGESTIONS_HEADING = 'Sugestões a partir do repositório'
export const REQUEST_SUGGESTIONS = [
  'Área do cliente com login',
  'Mapear o código',
  'Resolver as issues marcadas',
  'Subir a cobertura de testes',
] as const
export const REQUEST_ISSUES_HEADING = 'Issues abertas · kayke/portal-clientes'
export const REQUEST_ISSUES = [
  { num: '#12', title: 'Login por link mágico' },
  { num: '#15', title: 'Página de faturas' },
  { num: '#17', title: 'Status do pedido some no mobile' },
] as const
export const REQUEST_CONTEXT_HEADING = 'Contexto'
export const REQUEST_CONTEXT = [
  { label: 'Trazer memória de loja-online', hint: '18 fatos sobre pedidos e a API' },
  { label: 'Vincular a um plano', hint: 'Vokia — Plano do sistema · Docs' },
  { label: 'Arquivos', hint: 'arraste wireframes ou specs' },
] as const
export const REQUEST_START_HEADING = 'Quando começar'
export const REQUEST_START_OPTIONS = [
  { label: 'Planejar e esperar', hint: 'Iara propõe o plano e o 1º lote; ninguém codifica antes do seu ok' },
  { label: 'Começar já', hint: 'pula o ok da proposta só neste pedido; o padrão fica em Configurações › Execução' },
] as const
export const REQUEST_SEND_LABEL = 'Criar projeto e planejar'
export const NEXT_HEADING = 'O que acontece em seguida'
export const NEXT_STEPS = [
  { title: 'Clonar e preparar', text: 'clona o repo, cria a branch e o worktree' },
  { title: 'Iara lê o repositório', text: 'código, issues marcadas e a memória vinculada' },
  { title: 'Plano e primeiro lote', text: 'feature com cards por camada, no Plano' },
  { title: 'Você revisa', text: 'aprova o lote na barra de ação' },
  { title: 'Time começa', text: 'Davi e Nina pegam os cards' },
] as const
export const SUMMARY_HEADING = 'Resumo do projeto'
