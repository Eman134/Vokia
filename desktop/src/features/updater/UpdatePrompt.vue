<script setup lang="ts">
import { computed } from 'vue'
import { useUpdater } from './useUpdater'

const DATE_LOCALE = 'pt-BR'
const INSTALL_LABEL = 'Atualizar agora'
const DOWNLOADING_LABEL = 'Baixando...'
const INSTALLING_LABEL = 'Instalando...'

const updater = useUpdater()

const releaseDate = computed<string | null>(() => {
  if (updater.releaseDate === null) {
    return null
  }
  const parsed = new Date(updater.releaseDate)
  return Number.isNaN(parsed.getTime()) ? null : parsed.toLocaleDateString(DATE_LOCALE)
})

const installLabel = computed<string>(() => {
  if (updater.status === 'installing') {
    return INSTALLING_LABEL
  }
  if (updater.status === 'downloading') {
    return updater.progressPercent === null
      ? DOWNLOADING_LABEL
      : `${DOWNLOADING_LABEL} ${updater.progressPercent}%`
  }
  return INSTALL_LABEL
})
</script>

<template>
  <div
    v-if="updater.status !== 'idle'"
    class="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4"
  >
    <section
      role="dialog"
      aria-modal="true"
      aria-labelledby="update-prompt-title"
      class="w-130 max-w-full rounded-dialog border border-line bg-surface p-6 shadow-float"
    >
      <h2 id="update-prompt-title" class="text-section text-ink">Nova versão disponível</h2>
      <p class="mt-1 text-caption text-muted">
        Vokia {{ updater.version }} · instalada: {{ updater.currentVersion }}
      </p>
      <p v-if="releaseDate" class="text-caption text-muted">Publicada em {{ releaseDate }}</p>

      <div class="mt-4 max-h-64 overflow-y-auto rounded-control-lg bg-surface-2 p-3">
        <p v-if="updater.notes" class="whitespace-pre-wrap text-control text-ink">{{ updater.notes }}</p>
        <p v-else class="text-control text-muted">Sem notas de versão.</p>
      </div>

      <p v-if="updater.installError" class="mt-4 text-small text-danger">{{ updater.installError }}</p>

      <div class="mt-6 flex justify-end gap-2">
        <button
          v-if="updater.status === 'failed'"
          type="button"
          class="h-9 rounded-control bg-surface-2 px-3.5 text-control font-medium text-ink"
          @click="updater.dismissUpdate()"
        >
          Fechar
        </button>
        <template v-else>
          <button
            type="button"
            class="h-9 rounded-control px-3.5 text-control font-medium text-muted disabled:opacity-40"
            :disabled="updater.busy"
            @click="updater.dismissUpdate()"
          >
            Depois
          </button>
          <button
            type="button"
            class="h-9 rounded-control bg-ink px-3.5 text-control font-medium text-bg disabled:opacity-40"
            :disabled="updater.busy"
            @click="updater.install()"
          >
            {{ installLabel }}
          </button>
        </template>
      </div>
    </section>
  </div>

  <div
    v-if="updater.checkError"
    class="fixed top-6 right-6 z-50 flex max-w-sm items-start gap-3 rounded-popover border border-line bg-surface px-3.5 py-3 shadow-float"
  >
    <p class="text-caption text-muted">{{ updater.checkError }}</p>
    <button
      type="button"
      class="h-7.5 shrink-0 rounded-control-sm px-3.5 text-control font-medium text-muted"
      @click="updater.dismissCheckError()"
    >
      Fechar
    </button>
  </div>
</template>
