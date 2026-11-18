import { invoke } from '@tauri-apps/api/core'

const ROLE_LIST_COMMAND = 'role_list'
const ROLE_CREATE_COMMAND = 'role_create'
const ROLE_UPDATE_COMMAND = 'role_update'
const ROLE_DELETE_COMMAND = 'role_delete'

export interface Role {
  id: string
  name: string
  description: string
  instructions: string
}

export interface RoleDraft {
  name: string
  description: string
  instructions: string
}

export interface RoleEdit {
  description: string
  instructions: string
}

export function listRoles(): Promise<Role[]> {
  return invoke<Role[]>(ROLE_LIST_COMMAND)
}

export function createRole(draft: RoleDraft): Promise<Role> {
  return invoke<Role>(ROLE_CREATE_COMMAND, { draft })
}

export function updateRole(id: string, edit: RoleEdit): Promise<Role> {
  return invoke<Role>(ROLE_UPDATE_COMMAND, { id, edit })
}

export function deleteRole(id: string): Promise<void> {
  return invoke<void>(ROLE_DELETE_COMMAND, { id })
}
