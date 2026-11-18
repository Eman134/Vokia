<script setup lang="ts">
import { onMounted, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { deletePermissionRule, listPermissionRules, type PermissionRule } from './projectsApi'

const props = defineProps<{ projectId: number }>()

const LABEL = 'Permissões sempre permitidas'
const EMPTY_TEXT = 'Nenhuma. Ao escolher "Permitir sempre" num pedido, ela aparece aqui.'
const REMOVE_LABEL = 'Remover'

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
        <code class="min-w-0 flex-1 truncate font-mono text-[11.5px] text-muted" :title="rule.command">{{ rule.command }}</code>
        <UiButton variant="quiet" size="sm" @press="remove(rule)">{{ REMOVE_LABEL }}</UiButton>
      </li>
    </ul>
    <p v-if="error" class="text-caption text-danger">{{ error }}</p>
  </div>
</template>
