import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { createChannel, deleteChannel, listChannels, type Channel } from './channelsApi'
import type { Agent } from '../team/events'
import { useProjects } from '../projects/useProjects'

export const useChannels = defineStore('channels', () => {
  const channels = ref<Channel[]>([])
  const loadError = ref<string | null>(null)
  const projects = useProjects()

  async function load(): Promise<void> {
    const projectId = projects.selected?.id
    channels.value = []
    loadError.value = null
    if (projectId === undefined) {
      return
    }
    try {
      const listed = await listChannels(projectId)
      if (projects.selected?.id === projectId) {
        channels.value = listed
      }
    } catch (error) {
      if (projects.selected?.id === projectId) {
        loadError.value = String(error)
      }
    }
  }

  async function create(name: string, members: readonly Agent[]): Promise<Channel> {
    const projectId = requireProject()
    const channel = await createChannel(projectId, name, members)
    channels.value = [...channels.value, channel]
    return channel
  }

  async function remove(id: number): Promise<void> {
    const projectId = requireProject()
    await deleteChannel(projectId, id)
    channels.value = channels.value.filter((channel) => channel.id !== id)
  }

  function requireProject(): number {
    const projectId = projects.selected?.id
    if (projectId === undefined) {
      throw new Error('Nenhum projeto selecionado.')
    }
    return projectId
  }

  watch(() => projects.selected?.id, load, { immediate: true })

  return { channels, loadError, load, create, remove }
})
