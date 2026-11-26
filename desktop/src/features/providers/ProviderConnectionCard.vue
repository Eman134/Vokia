<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiSwitch from '../../ui/UiSwitch.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import type {
  ProviderAuthFlowSpec,
  ProviderDiagnosticState,
  ProviderEnvVarView,
  ProviderLoginState,
  ProviderWithStatus,
  TestProviderResult,
} from './providersApi'
import { useProviders } from './useProviders'

const props = withDefaults(
  defineProps<{
    provider: ProviderWithStatus
    selected?: boolean
    selectable?: boolean
    showSettingsControls?: boolean
  }>(),
  {
    selected: false,
    selectable: false,
    showSettingsControls: false,
  },
)

const emit = defineEmits<{
  select: [id: string]
}>()

const providersStore = useProviders()

const STATUS_BADGE_MAP: Record<ProviderDiagnosticState, { label: string; classes: string; dot: string }> = {
  connected: { label: 'Conectado', classes: 'bg-ok-soft text-ok', dot: 'bg-ok' },
  installed_signed_out: { label: 'Sem login', classes: 'bg-warn-soft text-warn-text', dot: 'bg-warn' },
  not_installed: { label: 'Não instalado', classes: 'bg-surface-2 text-muted', dot: 'bg-faint' },
  error: { label: 'Erro', classes: 'bg-surface-2 text-danger', dot: 'bg-danger' },
}

const SOURCE_LABELS: Record<string, string> = {
  path: 'PATH do sistema',
  'known-dir': 'Pasta padrão',
  explicit: 'Caminho explícito',
}

const diagnosticState = computed<ProviderDiagnosticState>(() => {
  if (props.provider.diagnosticState) return props.provider.diagnosticState
  if (!props.provider.installed || props.provider.connectionStatus === 'missing') return 'not_installed'
  if (props.provider.connectionStatus === 'error') return 'error'
  if (props.provider.connectionStatus === 'signed-out') return 'installed_signed_out'
  return 'connected'
})

const statusMeta = computed(() => STATUS_BADGE_MAP[diagnosticState.value])
const isEnabled = computed(() => props.provider.enabled !== false)
const sourceLabel = computed(() =>
  props.provider.executableSource ? SOURCE_LABELS[props.provider.executableSource] ?? props.provider.executableSource : null,
)

const connectExpanded = ref(false)
const selectedFlowId = ref<string>('')
const apiKeyInput = ref('')
const pasteCodeInput = ref('')
const loginState = ref<ProviderLoginState | null>(null)
const loginBusy = ref(false)
const loginError = ref<string | null>(null)
let pollTimer: number | null = null

const testing = ref(false)
const testResult = ref<TestProviderResult | null>(null)

const logoutConfirmOpen = ref(false)
const loggingOut = ref(false)
const logoutMessage = ref<string | null>(null)

const envExpanded = ref(false)
const envLoading = ref(false)
const envVars = ref<ProviderEnvVarView[]>([])
const envKeyInput = ref('')
const envValueInput = ref('')
const envSensitiveInput = ref(false)
const envSaving = ref(false)
const envError = ref<string | null>(null)

const flows = computed<ProviderAuthFlowSpec[]>(() => props.provider.authFlows ?? [])
const activeFlow = computed<ProviderAuthFlowSpec | undefined>(
  () => flows.value.find((f) => f.id === selectedFlowId.value) ?? flows.value[0],
)

const canConnect = computed(() => {
  if (diagnosticState.value === 'not_installed') return false
  return flows.value.length > 0
})

function stopPolling(): void {
  if (pollTimer !== null) {
    window.clearInterval(pollTimer)
    pollTimer = null
  }
}

function startPolling(): void {
  stopPolling()
  pollTimer = window.setInterval(async () => {
    try {
      const next = await providersStore.getLoginState(props.provider.id)
      if (!next) return
      loginState.value = next
      if (next.phase === 'completed') {
        stopPolling()
        connectExpanded.value = false
        await providersStore.checkStatus(props.provider.id).catch(() => {})
      } else if (next.phase === 'error' || next.phase === 'cancelled') {
        stopPolling()
      }
    } catch {
      stopPolling()
    }
  }, 1200)
}

onBeforeUnmount(() => stopPolling())

onMounted(() => {
  if (flows.value.length > 0) {
    selectedFlowId.value = flows.value[0].id
  }
})

function toggleConnect(): void {
  connectExpanded.value = !connectExpanded.value
  loginError.value = null
  if (connectExpanded.value && flows.value.length > 0 && !selectedFlowId.value) {
    selectedFlowId.value = flows.value[0].id
  }
}

async function runStartFlow(flow?: ProviderAuthFlowSpec): Promise<void> {
  const target = flow ?? activeFlow.value
  if (!target) return
  selectedFlowId.value = target.id
  loginBusy.value = true
  loginError.value = null
  try {
    const state = await providersStore.startLogin(
      props.provider.id,
      target.id,
      target.kind === 'api-key' ? apiKeyInput.value : null,
    )
    loginState.value = state
    if (target.kind === 'api-key') {
      apiKeyInput.value = ''
    }
    if (state.phase === 'completed') {
      connectExpanded.value = false
      await providersStore.checkStatus(props.provider.id).catch(() => {})
      if (envExpanded.value) await loadEnvVars()
    } else if (
      state.phase === 'awaiting-browser' ||
      state.phase === 'awaiting-device-code' ||
      state.phase === 'awaiting-paste-code'
    ) {
      startPolling()
    }
  } catch (err) {
    loginError.value = String(err)
  } finally {
    loginBusy.value = false
  }
}

async function submitCode(): Promise<void> {
  if (!pasteCodeInput.value.trim()) return
  loginBusy.value = true
  loginError.value = null
  try {
    const code = pasteCodeInput.value.trim()
    pasteCodeInput.value = ''
    loginState.value = await providersStore.submitLoginCode(props.provider.id, code)
  } catch (err) {
    loginError.value = String(err)
  } finally {
    loginBusy.value = false
  }
}

async function cancelFlow(): Promise<void> {
  stopPolling()
  try {
    loginState.value = await providersStore.cancelLogin(props.provider.id)
  } catch (err) {
    loginError.value = String(err)
  }
}

async function runCheckStatus(): Promise<void> {
  await providersStore.checkStatus(props.provider.id).catch(() => {})
}

async function runTest(): Promise<void> {
  testing.value = true
  testResult.value = null
  try {
    const model = props.provider.tiers.intermediate || props.provider.defaultModel || undefined
    testResult.value = await providersStore.testConfig(props.provider, model)
  } catch (err) {
    testResult.value = { ok: false, latencyMs: 0, reply: '', errorKind: 'other', errorMessage: String(err) }
  } finally {
    testing.value = false
  }
}

async function toggleEnabled(next: boolean): Promise<void> {
  await providersStore.setEnabled(
    props.provider.id,
    next,
    next ? null : 'Desativado pelo usuário em Configurações.',
  )
}

async function confirmLogout(): Promise<void> {
  loggingOut.value = true
  logoutMessage.value = null
  try {
    const res = await providersStore.logout(props.provider.id)
    logoutMessage.value = res.message
    logoutConfirmOpen.value = false
    loginState.value = null
    if (envExpanded.value) await loadEnvVars()
  } catch (err) {
    loginError.value = String(err)
  } finally {
    loggingOut.value = false
  }
}

async function toggleEnvSection(): Promise<void> {
  envExpanded.value = !envExpanded.value
  if (envExpanded.value) {
    await loadEnvVars()
  }
}

async function loadEnvVars(): Promise<void> {
  envLoading.value = true
  envError.value = null
  try {
    envVars.value = await providersStore.listEnv(props.provider.id)
  } catch (err) {
    envError.value = String(err)
  } finally {
    envLoading.value = false
  }
}

function applyEnvPreset(key: string, sensitive?: boolean): void {
  envKeyInput.value = key
  envSensitiveInput.value = Boolean(sensitive)
  envValueInput.value = ''
  envError.value = null
}

async function saveEnvVar(): Promise<void> {
  if (!envKeyInput.value.trim() || !envValueInput.value.trim()) return
  envSaving.value = true
  envError.value = null
  try {
    await providersStore.setEnv(
      props.provider.id,
      envKeyInput.value.trim(),
      envValueInput.value,
      envSensitiveInput.value,
    )
    envKeyInput.value = ''
    envValueInput.value = ''
    envSensitiveInput.value = false
    await loadEnvVars()
  } catch (err) {
    envError.value = String(err)
  } finally {
    envSaving.value = false
  }
}

async function removeEnvVar(key: string): Promise<void> {
  envError.value = null
  try {
    await providersStore.deleteEnv(props.provider.id, key)
    await loadEnvVars()
  } catch (err) {
    envError.value = String(err)
  }
}
</script>

<template>
  <article
    :data-testid="`provider-card-${provider.id}`"
    class="flex flex-col gap-3 rounded-md border bg-surface p-4 transition-colors"
    :class="selected ? 'border-accent ring-1 ring-accent' : 'border-line'"
  >
    <div class="flex flex-wrap items-start justify-between gap-3">
      <div class="flex min-w-0 flex-1 items-start gap-2.5">
        <span
          v-if="provider.badgeCode"
          class="grid size-8 shrink-0 place-items-center rounded-sm bg-surface-2 font-mono text-small font-bold text-ink"
        >
          {{ provider.badgeCode }}
        </span>
        <div class="flex min-w-0 flex-1 flex-col gap-1">
          <div class="flex flex-wrap items-center gap-2">
            <span class="truncate text-control font-semibold text-ink">{{ provider.label }}</span>
            <span
              class="inline-flex h-5 items-center gap-1.5 rounded-sm px-2 text-small font-medium"
              :class="statusMeta.classes"
            >
              <span class="size-1.5 rounded-full" :class="statusMeta.dot" aria-hidden="true" />
              {{ statusMeta.label }}
            </span>
            <span v-if="provider.version" class="font-mono text-tiny text-muted">
              {{ provider.version }}
            </span>
            <span v-if="sourceLabel" class="text-tiny text-faint">
              · {{ sourceLabel }}
            </span>
          </div>

          <p v-if="!isEnabled" class="text-small text-warn-text">
            Desativado{{ provider.disabledReason ? `: ${provider.disabledReason}` : '' }}
          </p>
          <p v-else-if="diagnosticState === 'not_installed'" class="text-small text-muted">
            {{ provider.installHint ?? 'CLI não encontrada no PATH.' }}
          </p>
          <p v-else-if="diagnosticState === 'error' && provider.statusDetail" class="text-small text-danger">
            {{ provider.statusDetail }}
          </p>
          <p v-else-if="provider.statusDetail" class="text-small text-muted">
            {{ provider.statusDetail }}
          </p>
        </div>
      </div>

      <div class="flex shrink-0 flex-wrap items-center gap-1.5">
        <UiButton
          v-if="selectable"
          :data-testid="`select-provider-${provider.id}`"
          :variant="selected ? 'primary' : 'quiet'"
          size="sm"
          :disabled="!isEnabled"
          @press="emit('select', provider.id)"
        >
          {{ selected ? 'Selecionado' : 'Usar este' }}
        </UiButton>

        <UiButton
          v-if="canConnect"
          :data-testid="`connect-provider-${provider.id}`"
          :variant="diagnosticState === 'installed_signed_out' ? 'primary' : 'quiet'"
          size="sm"
          @press="toggleConnect"
        >
          {{ diagnosticState === 'connected' ? 'Reconectar' : 'Conectar' }}
        </UiButton>

        <UiButton
          variant="quiet"
          size="sm"
          :disabled="providersStore.checkingStatus[provider.id]"
          @press="runCheckStatus"
        >
          {{ providersStore.checkingStatus[provider.id] ? 'Verificando...' : 'Verificar' }}
        </UiButton>

        <UiButton
          variant="quiet"
          size="sm"
          :disabled="testing || !provider.installed"
          @press="runTest"
        >
          {{ testing ? 'Testando...' : 'Testar' }}
        </UiButton>

        <UiButton variant="quiet" size="sm" @press="providersStore.openSetup(provider.id)">
          Editar
        </UiButton>
      </div>
    </div>

    <p v-if="testResult" class="text-small" :class="testResult.ok ? 'text-ok' : 'text-danger'">
      {{ testResult.ok ? `Teste OK (${testResult.latencyMs} ms)` : testResult.errorMessage ?? 'Falha no teste' }}
    </p>
    <p v-if="logoutMessage" class="text-small text-ok">{{ logoutMessage }}</p>

    <!-- Painel de conexão assistida declarativa -->
    <div
      v-if="connectExpanded"
      :data-testid="`connect-panel-${provider.id}`"
      class="flex flex-col gap-3 rounded-md border border-line bg-surface-2 p-3"
      @keydown.esc.stop="connectExpanded = false"
    >
      <div class="flex flex-wrap gap-1.5">
        <button
          v-for="flow in flows"
          :key="flow.id"
          type="button"
          class="cursor-pointer rounded-sm border px-2.5 py-1 text-small font-medium transition-colors"
          :class="[
            activeFlow?.id === flow.id
              ? 'border-accent bg-accent-soft text-accent'
              : 'border-line bg-surface text-ink hover:bg-hover',
            INK_FOCUS_CLASSES,
          ]"
          @click="selectedFlowId = flow.id"
        >
          {{ flow.label }}
        </button>
      </div>

      <p v-if="activeFlow?.description" class="text-small text-muted">
        {{ activeFlow.description }}
      </p>

      <div v-if="activeFlow?.kind === 'api-key'" class="flex flex-wrap items-center gap-2">
        <input
          v-model="apiKeyInput"
          type="password"
          autocomplete="off"
          :placeholder="`Cole ${activeFlow.envVar ?? 'a chave'}`"
          class="h-8 min-w-52 flex-1 rounded-md border border-line bg-surface px-2.5 text-caption text-ink"
          :class="INK_FOCUS_CLASSES"
        />
        <UiButton
          variant="primary"
          size="sm"
          :disabled="loginBusy || !apiKeyInput.trim()"
          @press="runStartFlow(activeFlow)"
        >
          {{ loginBusy ? 'Salvando...' : 'Salvar chave' }}
        </UiButton>
      </div>

      <div v-else-if="activeFlow" class="flex flex-wrap items-center gap-2">
        <UiButton
          variant="primary"
          size="sm"
          :disabled="loginBusy"
          @press="runStartFlow(activeFlow)"
        >
          {{ loginBusy ? 'Iniciando...' : activeFlow.kind === 'free-no-login' ? 'Ativar modo gratuito' : 'Iniciar conexão' }}
        </UiButton>
        <UiButton
          v-if="loginState && (loginState.phase === 'awaiting-browser' || loginState.phase === 'awaiting-device-code' || loginState.phase === 'awaiting-paste-code')"
          variant="quiet"
          size="sm"
          @press="cancelFlow"
        >
          Cancelar
        </UiButton>
      </div>

      <div
        v-if="loginState && loginState.phase !== 'cancelled'"
        class="flex flex-col gap-2 rounded-sm border border-line bg-surface p-2.5 text-small"
      >
        <p :class="loginState.phase === 'error' ? 'text-danger' : loginState.phase === 'completed' ? 'text-ok' : 'text-ink'">
          {{ loginState.message }}
        </p>
        <div v-if="loginState.url" class="flex flex-wrap items-center gap-2">
          <span class="text-muted">Link:</span>
          <a
            :href="loginState.url"
            target="_blank"
            rel="noopener noreferrer"
            class="cursor-pointer font-mono text-accent underline"
          >
            {{ loginState.url }}
          </a>
        </div>
        <div v-if="loginState.userCode" class="flex items-center gap-2">
          <span class="text-muted">Código curto:</span>
          <code class="rounded-sm bg-surface-2 px-2 py-0.5 font-mono text-control font-bold text-ink">
            {{ loginState.userCode }}
          </code>
        </div>
        <div v-if="loginState.phase === 'awaiting-paste-code'" class="flex flex-wrap items-center gap-2">
          <input
            v-model="pasteCodeInput"
            type="password"
            autocomplete="off"
            placeholder="Cole o código de autorização"
            class="h-8 min-w-52 flex-1 rounded-md border border-line bg-surface px-2.5 text-caption text-ink"
            :class="INK_FOCUS_CLASSES"
          />
          <UiButton variant="primary" size="sm" :disabled="loginBusy || !pasteCodeInput.trim()" @press="submitCode">
            Enviar código
          </UiButton>
        </div>
      </div>

      <p v-if="loginError" class="text-small text-danger">{{ loginError }}</p>
    </div>

    <!-- Controles avançados em Configurações -->
    <div v-if="showSettingsControls" class="flex flex-wrap items-center justify-between gap-2 border-t border-line pt-2.5">
      <div class="flex items-center gap-2">
        <UiSwitch
          :model-value="isEnabled"
          :label="`Ativar ${provider.label}`"
          @update:model-value="toggleEnabled"
        />
        <span class="text-small text-muted">{{ isEnabled ? 'Ativo' : 'Desativado' }}</span>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <UiButton
          :data-testid="`toggle-env-${provider.id}`"
          variant="quiet"
          size="sm"
          @press="toggleEnvSection"
        >
          {{ envExpanded ? 'Ocultar ambiente' : 'Ambiente' }}
        </UiButton>
        <UiButton
          v-if="provider.logout && diagnosticState === 'connected'"
          :data-testid="`logout-provider-${provider.id}`"
          variant="danger"
          size="sm"
          @press="logoutConfirmOpen = true"
        >
          Desconectar
        </UiButton>
      </div>
    </div>

    <div
      v-if="showSettingsControls && logoutConfirmOpen"
      class="flex flex-col gap-2 rounded-md border border-line bg-warn-soft p-3 text-small"
    >
      <p class="font-medium text-warn-text">
        {{ provider.logout?.warning ?? `Deseja desconectar ${provider.label}?` }}
      </p>
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="sm" @press="logoutConfirmOpen = false">Cancelar</UiButton>
        <UiButton variant="danger" size="sm" :disabled="loggingOut" @press="confirmLogout">
          {{ loggingOut ? 'Desconectando...' : 'Confirmar desconexão' }}
        </UiButton>
      </div>
    </div>

    <!-- Seção Ambiente (variáveis e gateways opcionais) -->
    <div
      v-if="showSettingsControls && envExpanded"
      :data-testid="`env-section-${provider.id}`"
      class="flex flex-col gap-3 rounded-md border border-line bg-surface-2 p-3"
    >
      <div class="flex flex-wrap items-center justify-between gap-2">
        <span class="text-small font-semibold text-ink">Variáveis de ambiente &amp; gateways</span>
        <div v-if="(provider.envPresets?.length ?? 0) > 0" class="flex flex-wrap gap-1">
          <button
            v-for="preset in provider.envPresets"
            :key="preset.key"
            type="button"
            class="cursor-pointer rounded-sm border border-line bg-surface px-2 py-0.5 font-mono text-tiny text-ink hover:bg-hover"
            :class="INK_FOCUS_CLASSES"
            @click="applyEnvPreset(preset.key, preset.sensitive)"
          >
            + {{ preset.key }}
          </button>
        </div>
      </div>

      <p v-if="envLoading" class="text-small text-muted">Carregando variáveis...</p>
      <ul v-else-if="envVars.length > 0" class="flex flex-col divide-y divide-line rounded-sm border border-line bg-surface">
        <li
          v-for="item in envVars"
          :key="item.key"
          class="flex items-center justify-between gap-2 px-2.5 py-1.5 text-small"
        >
          <div class="flex min-w-0 items-center gap-2">
            <code class="font-mono font-semibold text-ink">{{ item.key }}</code>
            <span v-if="item.sensitive" class="rounded-sm bg-ok-soft px-1.5 py-0.5 text-tiny text-ok">
              {{ item.stored ? 'salvo no cofre' : item.status }}
            </span>
            <span v-else class="truncate font-mono text-muted">{{ item.value }}</span>
          </div>
          <UiButton variant="quiet" size="sm" @press="removeEnvVar(item.key)">Remover</UiButton>
        </li>
      </ul>
      <p v-else class="text-small text-muted">Nenhuma variável configurada.</p>

      <div class="flex flex-wrap items-center gap-2">
        <input
          v-model="envKeyInput"
          type="text"
          placeholder="NOME_DA_VARIAVEL"
          class="h-8 w-44 rounded-md border border-line bg-surface px-2 font-mono text-small text-ink"
          :class="INK_FOCUS_CLASSES"
        />
        <input
          v-model="envValueInput"
          :type="envSensitiveInput ? 'password' : 'text'"
          autocomplete="off"
          placeholder="Valor"
          class="h-8 min-w-40 flex-1 rounded-md border border-line bg-surface px-2 text-small text-ink"
          :class="INK_FOCUS_CLASSES"
        />
        <label class="flex cursor-pointer items-center gap-1.5 text-small text-ink">
          <input v-model="envSensitiveInput" type="checkbox" class="cursor-pointer" />
          Sensível
        </label>
        <UiButton
          variant="primary"
          size="sm"
          :disabled="envSaving || !envKeyInput.trim() || !envValueInput.trim()"
          @press="saveEnvVar"
        >
          {{ envSaving ? 'Salvando...' : 'Salvar' }}
        </UiButton>
      </div>
      <p v-if="envError" class="text-small text-danger">{{ envError }}</p>
    </div>
  </article>
</template>
