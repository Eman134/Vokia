import { ref } from 'vue'
import { defineStore } from 'pinia'
import { createRole, deleteRole, listRoles, updateRole, type Role, type RoleDraft, type RoleEdit } from './rolesApi'

export const useRoles = defineStore('roles', () => {
  const roles = ref<Role[]>([])

  async function load(): Promise<void> {
    roles.value = await listRoles()
  }

  async function create(draft: RoleDraft): Promise<Role> {
    const role = await createRole(draft)
    roles.value = [...roles.value, role]
    return role
  }

  async function update(id: string, edit: RoleEdit): Promise<Role> {
    const role = await updateRole(id, edit)
    roles.value = roles.value.map((current) => (current.id === id ? role : current))
    return role
  }

  async function remove(id: string): Promise<void> {
    await deleteRole(id)
    roles.value = roles.value.filter((current) => current.id !== id)
  }

  return { roles, load, create, update, remove }
})
