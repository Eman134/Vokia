<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiCloseButton from '../../ui/UiCloseButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import { deletePermissionRule, listPermissionRules, type PermissionRule } from './projectsApi'

const props = defineProps<{ projectId: number }>()

const LABEL = 'Permissões sempre permitidas'
const EMPTY_TEXT = 'Nenhuma. Ao escolher "Permitir sempre" num pedido, ela aparece aqui.'
const REMOVE_LABEL = 'Remover'
const REMOVE_MESSAGE = 'O comando volta a pedir permissão neste projeto.'
const REMOVE_CONFIRM_LABEL = 'Remover permissão'

const rules = ref<PermissionRule[]>([])
const error = ref<string | null>(null)
const modalOpen = ref(false)

function formatLastUsed(rule: PermissionRule): string {
  const ts = rule.lastUsedAt ?? rule.createdAt
  if (!ts) return 'Sem registro'
  const ms = ts > 10_000_000_000 ? ts : ts * 1000
  const diffMin = Math.max(0, Math.floor((Date.now() - ms) / 60_000))
  const prefix = rule.lastUsedAt ? 'Último uso' : 'Criada'
  if (diffMin < 1) return `${prefix}: agora`
  if (diffMin < 60) return `${prefix}: há ${diffMin} min`
  const diffH = Math.floor(diffMin / 60)
  if (diffH < 24) return `${prefix}: há ${diffH} h`
  const diffD = Math.floor(diffH / 24)
  return `${prefix}: há ${diffD} d`
}

async function load(): Promise<void> {
  try {
    rules.value = await listPermissionRules(props.projectId)
  } catch (cause) {
    error.value = String(cause)
  }
}

async function openModal(): Promise<void> {
  modalOpen.value = true
  await load()
}

async function remove(rule: PermissionRule): Promise<void> {
  try {
    await deletePermissionRule(props.projectId, rule)
    rules.value = rules.value.filter((item) => item !== rule)
  } catch (cause) {
    error.value = String(cause)
  }
}

function onKeydown(e: KeyboardEvent): void {
  if (modalOpen.value && e.key === 'Escape') {
    e.stopPropagation()
    modalOpen.value = false
  }
}

onMounted(() => {
  void load()
  window.addEventListener('keydown', onKeydown)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div class="flex items-center justify-between gap-2" data-testid="permission-rules">
    <div class="min-w-0">
      <span class="text-label font-medium text-muted">{{ LABEL }}</span>
      <p class="text-tiny text-muted">
        {{ rules.length === 0 ? 'Nenhuma regra salva' : `${rules.length} ${rules.length === 1 ? 'regra ativa' : 'regras ativas'}` }}
      </p>
    </div>
    <UiButton variant="quiet" size="sm" data-testid="open-permissions-modal" @press="openModal">
      Permissões ({{ rules.length }})
    </UiButton>
  </div>

  <Teleport to="body">
    <div
      v-if="modalOpen"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      role="dialog"
      aria-modal="true"
      aria-label="Permissões do projeto"
      @click.self="modalOpen = false"
    >
      <section class="flex max-h-[70vh] w-full max-w-lg flex-col overflow-hidden rounded-lg border border-line bg-surface shadow-lg">
        <header class="flex items-center justify-between gap-3 border-b border-line px-4 py-3">
          <div class="min-w-0">
            <h2 class="text-label font-semibold text-ink">{{ LABEL }}</h2>
            <p class="text-tiny text-muted">Ordenadas pelo último uso neste projeto.</p>
          </div>
          <UiCloseButton @press="modalOpen = false" />
        </header>
        <div class="flex-1 overflow-y-auto p-4">
          <p v-if="error" class="mb-2 text-caption text-danger">{{ error }}</p>
          <p v-if="rules.length === 0" class="text-caption text-muted">{{ EMPTY_TEXT }}</p>
          <ul v-else class="flex flex-col gap-2">
            <li
              v-for="rule in rules"
              :key="`${rule.tool}:${rule.command}`"
              class="flex items-center gap-2 rounded-sm border border-line/70 bg-surface-2 px-2.5 py-1.5"
            >
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-1.5">
                  <span class="shrink-0 text-caption font-semibold text-ink">{{ rule.tool }}</span>
                  <code class="min-w-0 flex-1 truncate font-mono text-meta text-muted" :title="rule.command">{{ rule.command }}</code>
                </div>
                <p class="text-tiny text-muted tabular-nums">{{ formatLastUsed(rule) }}</p>
              </div>
              <UiConfirm :label="REMOVE_LABEL" :message="REMOVE_MESSAGE" :confirm-label="REMOVE_CONFIRM_LABEL" @confirm="remove(rule)" />
            </li>
          </ul>
        </div>
      </section>
    </div>
  </Teleport>
</template>
