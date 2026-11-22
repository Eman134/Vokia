<script setup lang="ts">
import { onMounted, ref } from 'vue'
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

async function load(): Promise<void> {
  try {
    rules.value = await listPermissionRules(props.projectId)
  } catch (cause) {
    error.value = String(cause)
  }
}

async function remove(rule: PermissionRule): Promise<void> {
  try {
    await deletePermissionRule(props.projectId, rule)
    rules.value = rules.value.filter((item) => item !== rule)
  } catch (cause) {
    error.value = String(cause)
  }
}

onMounted(load)
</script>

<template>
  <div class="flex flex-col gap-1.5" data-testid="permission-rules">
    <span class="text-label font-medium text-muted">{{ LABEL }}</span>
    <p v-if="rules.length === 0" class="text-caption text-muted">{{ EMPTY_TEXT }}</p>
    <ul v-else class="flex flex-col gap-1.5">
      <li v-for="rule in rules" :key="`${rule.tool}:${rule.command}`" class="flex items-center gap-2">
        <span class="shrink-0 text-caption font-semibold text-ink">{{ rule.tool }}</span>
        <code class="min-w-0 flex-1 truncate font-mono text-meta text-muted" :title="rule.command">{{ rule.command }}</code>
        <UiConfirm :label="REMOVE_LABEL" :message="REMOVE_MESSAGE" :confirm-label="REMOVE_CONFIRM_LABEL" @confirm="remove(rule)" />
      </li>
    </ul>
    <p v-if="error" class="text-caption text-danger">{{ error }}</p>
  </div>
</template>
