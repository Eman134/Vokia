<script setup lang="ts">
import { agentName, DEFAULT_RECIPIENT } from './agents'
import { useRequests, type QueuedRequest } from './useRequests'
import { QUEUE_EDIT, QUEUE_REMOVE, QUEUED_TAG } from './queueCopy'
import QueueItemEditor from './QueueItemEditor.vue'
import UiButton from '../../ui/UiButton.vue'
import UserAvatar from '../profile/UserAvatar.vue'
import { useUserProfile } from '../profile/useUserProfile'

const USER_AVATAR_SIZE = 28

defineProps<{ request: QueuedRequest }>()

const requests = useRequests()
const userProfile = useUserProfile()
</script>

<template>
  <div class="flex items-start justify-end gap-2.5 opacity-60">
    <div class="flex max-w-3/4 flex-col items-end gap-1">
      <span class="flex items-center gap-2 text-caption text-muted">
        <span class="rounded-control bg-surface-2 px-1.5 font-semibold">{{ QUEUED_TAG }}</span>
        <span v-if="request.to !== DEFAULT_RECIPIENT">Para {{ agentName(request.to) }}</span>
      </span>
      <QueueItemEditor v-if="request.editing" :request="request" class="w-full" />
      <template v-else>
        <p
          class="wrap-break-word whitespace-pre-wrap rounded-card rounded-tr-tail bg-surface-2 px-4 py-3 text-control text-muted"
        >
          {{ request.text }}
        </p>
        <div class="flex gap-1">
          <UiButton variant="quiet" size="sm" @press="requests.startEdit(request.id)">{{ QUEUE_EDIT }}</UiButton>
          <UiButton variant="quiet" size="sm" @press="requests.removeQueued(request.id)">{{ QUEUE_REMOVE }}</UiButton>
        </div>
      </template>
    </div>
    <UserAvatar :size="USER_AVATAR_SIZE" :title="userProfile.displayName" />
  </div>
</template>
