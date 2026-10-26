<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useTeam } from './stores/useTeam'
import AppRail from './components/AppRail.vue'
import AgentList from './components/AgentList.vue'
import Conversation from './components/Conversation.vue'
import ActionBar from './components/ActionBar.vue'
import UpdatePrompt from './components/UpdatePrompt.vue'
import { useUpdater } from './stores/useUpdater'

const team = useTeam()
const updater = useUpdater()

onMounted(team.start)
onMounted(updater.check)
onUnmounted(team.stop)
</script>

<template>
  <main
    v-if="team.startupError"
    class="flex h-screen items-center justify-center bg-bg p-8 text-center text-body text-danger"
  >
    {{ team.startupError }}
  </main>
  <main
    v-else-if="team.cliProblem"
    class="flex h-screen flex-col items-center justify-center gap-4 bg-bg p-8 text-center text-ink"
  >
    <h1 class="text-section">A Vokia precisa da CLI do Claude Code</h1>
    <p class="max-w-lg text-body text-muted">{{ team.cliProblem.reason }}</p>
    <p class="text-body text-muted">Para resolver, rode no terminal e reabra o app:</p>
    <code class="rounded-control-sm bg-surface-2 px-2.5 py-2 font-mono text-caption text-ink">
      {{ team.cliProblem.fixCommand }}
    </code>
  </main>
  <div v-else-if="team.cliChecked" class="flex h-screen bg-bg text-ink">
    <AppRail />
    <AgentList />
    <Conversation />
    <ActionBar />
  </div>
  <UpdatePrompt />
</template>
