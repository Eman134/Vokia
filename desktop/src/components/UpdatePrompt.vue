<script setup lang="ts">
import { computed } from 'vue'
import { useUpdater } from '../stores/useUpdater'

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
    class="fixed inset-0 z-50 flex items-center justify-center bg-canvas/70 p-4"
  >
    <section
      role="dialog"
      aria-modal="true"
      aria-labelledby="update-prompt-title"
      class="w-[min(520px,100%)] rounded-2xl border border-line bg-surface p-6 shadow-(--shadow-float)"
    >
      <h2 id="update-prompt-title" class="text-lg font-semibold text-ink">Nova versão disponível</h2>
      <p class="mt-1 text-sm text-muted">
        Vokia {{ updater.version }} · instalada: {{ updater.currentVersion }}
      </p>
      <p v-if="releaseDate" class="text-sm text-muted">Publicada em {{ releaseDate }}</p>

      <div class="mt-4 max-h-64 overflow-y-auto rounded-xl border border-line bg-canvas p-3">
        <p v-if="updater.notes" class="whitespace-pre-wrap text-sm text-ink">{{ updater.notes }}</p>
        <p v-else class="text-sm text-muted">Sem notas de versão.</p>
      </div>

      <p v-if="updater.installError" class="mt-4 text-sm text-danger">{{ updater.installError }}</p>

      <div class="mt-6 flex justify-end gap-2">
        <button
          v-if="updater.status === 'failed'"
          type="button"
          class="rounded-xl bg-surface-2 px-4 py-2 text-sm font-medium text-ink"
          @click="updater.dismissUpdate()"
        >
          Fechar
        </button>
        <template v-else>
          <button
            type="button"
            class="rounded-xl bg-surface-2 px-4 py-2 text-sm font-medium text-ink disabled:opacity-60"
            :disabled="updater.busy"
            @click="updater.dismissUpdate()"
          >
            Depois
          </button>
          <button
            type="button"
            class="rounded-xl bg-accent px-4 py-2 text-sm font-medium text-accent-ink disabled:opacity-60"
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
    class="fixed top-6 right-6 z-50 flex max-w-sm items-start gap-3 rounded-2xl border border-line bg-surface px-4 py-3 shadow-(--shadow-float)"
  >
    <p class="text-sm text-muted">{{ updater.checkError }}</p>
    <button
      type="button"
      class="shrink-0 text-sm font-medium text-accent"
      @click="updater.dismissCheckError()"
    >
      Fechar
    </button>
  </div>
</template>
