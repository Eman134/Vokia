import { invoke } from '@tauri-apps/api/core'
import type { Agent } from '../team/events'

const LIST_CHANNELS_COMMAND = 'list_channels'
const CREATE_CHANNEL_COMMAND = 'create_channel'
const DELETE_CHANNEL_COMMAND = 'delete_channel'

export interface Channel {
  id: number
  projectId: number
  name: string
  members: Agent[]
  createdAt: number
}

export function listChannels(projectId: number): Promise<Channel[]> {
  return invoke<Channel[]>(LIST_CHANNELS_COMMAND, { projectId })
}

export function createChannel(projectId: number, name: string, members: readonly Agent[]): Promise<Channel> {
  return invoke<Channel>(CREATE_CHANNEL_COMMAND, { projectId, name, members })
}

export function deleteChannel(projectId: number, id: number): Promise<void> {
  return invoke<void>(DELETE_CHANNEL_COMMAND, { projectId, id })
}
