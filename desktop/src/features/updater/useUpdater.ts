import { computed, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'
import { checkForUpdate, relaunchApp, type DownloadEvent, type Update } from './updaterApi'

const UPDATE_CHECK_TIMEOUT_MS = 10_000
const PERCENT = 100

export type UpdateStatus = 'idle' | 'available' | 'downloading' | 'installing' | 'ready' | 'failed'

export const useUpdater = defineStore('updater', () => {
  const status = ref<UpdateStatus>('idle')
  // shallowRef: o Update é um recurso do backend, não deve ser envolvido pelo proxy reativo.
  const update = shallowRef<Update | null>(null)
  const downloadedBytes = ref(0)
  const totalBytes = ref<number | null>(null)
  const checkError = ref<string | null>(null)
  const installError = ref<string | null>(null)

  const busy = computed<boolean>(() => status.value === 'downloading' || status.value === 'installing')
  const version = computed<string | null>(() => update.value?.version ?? null)
  const currentVersion = computed<string | null>(() => update.value?.currentVersion ?? null)
  const releaseDate = computed<string | null>(() => update.value?.date ?? null)
  const notes = computed<string | null>(() => update.value?.body?.trim() ?? null)
  const readyText = computed<string | null>(() =>
    version.value === null ? null : `Versão ${version.value} pronta`,
  )
  const progressPercent = computed<number | null>(() =>
    totalBytes.value ? Math.round((downloadedBytes.value / totalBytes.value) * PERCENT) : null,
  )

  async function check(): Promise<void> {
    if (!import.meta.env.PROD) {
      return
    }
    try {
      const found = await checkForUpdate(UPDATE_CHECK_TIMEOUT_MS)
      if (found !== null) {
        update.value = found
        status.value = 'available'
      }
    } catch (error) {
      checkError.value = `Não foi possível verificar atualizações: ${String(error)}`
    }
  }

  function trackDownload(event: DownloadEvent): void {
    switch (event.event) {
      case 'Started':
        totalBytes.value = event.data.contentLength ?? null
        break
      case 'Progress':
        downloadedBytes.value += event.data.chunkLength
        break
    }
  }

  async function install(): Promise<void> {
    const current = update.value
    if (current === null || busy.value) {
      return
    }
    status.value = 'downloading'
    downloadedBytes.value = 0
    totalBytes.value = null
    try {
      await current.download(trackDownload)
      status.value = 'installing'
      await current.install()
      status.value = 'ready'
    } catch (error) {
      status.value = 'failed'
      installError.value = `Não foi possível atualizar: ${String(error)}`
    }
  }

  async function restart(): Promise<void> {
    try {
      await relaunchApp()
    } catch (error) {
      installError.value = `Não foi possível reiniciar: ${String(error)}`
    }
  }

  async function dismissUpdate(): Promise<void> {
    const current = update.value
    update.value = null
    status.value = 'idle'
    installError.value = null
    try {
      await current?.close()
    } catch (error) {
      console.error(`Não foi possível liberar a atualização: ${String(error)}`)
    }
  }

  function dismissCheckError(): void {
    checkError.value = null
  }

  return {
    status,
    busy,
    version,
    currentVersion,
    releaseDate,
    notes,
    readyText,
    progressPercent,
    checkError,
    installError,
    check,
    install,
    restart,
    dismissUpdate,
    dismissCheckError,
  }
})
