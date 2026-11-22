<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import { useNavigation } from '../../shell/useNavigation'
import { useChannels } from '../channels/useChannels'
import { agentName } from './agents'
import { useComposerSubmit } from './bar/useComposerSubmit'
import { screenPlaceholder } from './screenRecipient'
import { COMMAND_TRIGGER, SLASH_COMMANDS } from './slashCommands'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'
import { useRoster } from './useRoster'

const FALLBACK_PLACEHOLDER = 'Mensagem para o time'
const FIELD_LABEL = 'Mensagem rápida'
const SEND_LABEL = 'Enviar'
const COMMANDS_LABEL = 'Comandos'
const FOOTER_CLASSES = 'flex shrink-0 flex-col gap-1.5 border-t border-line bg-paper px-4 py-2.5'
const FIELD_CLASSES = `field-sizing-content max-h-28 min-h-8 min-w-0 flex-1 resize-none rounded-md border border-line bg-surface px-2.5 py-1.5 text-control text-ink placeholder:text-faint ${INK_FOCUS_CLASSES}`
const CHIP_CLASSES = `cursor-pointer rounded-sm border border-line px-2 py-0.5 font-mono text-meta text-muted hover:bg-hover hover:text-ink ${INK_FOCUS_CLASSES}`

// Só o que tem texto pronto para o agente: /pausar e /lembrar ainda não existem e /setup não é de conversa.
const chips = SLASH_COMMANDS.filter((command) => command.enabled && command.prompt !== undefined)

const connection = useConnection()
const feed = useFeed()
const roster = useRoster()
const channels = useChannels()
const navigation = useNavigation()
const { draft, submit } = useComposerSubmit()
const field = ref<HTMLTextAreaElement>()

const rosterOrder = computed(() => roster.agents.map((profile) => profile.id))
const placeholder = computed(
  () => screenPlaceholder(navigation.page, feed.screen, agentName, channels.channels, rosterOrder.value) ?? FALLBACK_PLACEHOLDER,
)
const canSend = computed(() => draft.value.trim() !== '' && !connection.checking)

// Enter envia, Shift+Enter quebra a linha; durante a composição de acento o Enter confirma a letra, não envia.
function onEnter(event: KeyboardEvent): void {
  if (event.isComposing || event.shiftKey) return
  event.preventDefault()
  void submit()
}

async function useChip(id: string): Promise<void> {
  draft.value = `${COMMAND_TRIGGER}${id} `
  await nextTick()
  field.value?.focus()
}
</script>

<template>
  <div :class="FOOTER_CLASSES" data-testid="conversation-composer">
    <div role="group" :aria-label="COMMANDS_LABEL" class="flex flex-wrap items-center gap-1.5">
      <button
        v-for="command in chips"
        :key="command.id"
        type="button"
        :class="CHIP_CLASSES"
        :title="command.description"
        @click="useChip(command.id)"
      >
        {{ COMMAND_TRIGGER }}{{ command.id }}
      </button>
    </div>
    <div class="flex items-end gap-2">
      <textarea
        ref="field"
        v-model="draft"
        rows="1"
        :aria-label="FIELD_LABEL"
        :placeholder="placeholder"
        :class="FIELD_CLASSES"
        data-testid="conversation-composer-field"
        @keydown.enter="onEnter"
      />
      <UiButton variant="primary" size="lg" :disabled="!canSend" @press="submit">{{ SEND_LABEL }}</UiButton>
    </div>
  </div>
</template>
