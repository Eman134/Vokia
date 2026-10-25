<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useTeam } from './stores/useTeam'
import AppRail from './components/AppRail.vue'
import AgentList from './components/AgentList.vue'
import Conversation from './components/Conversation.vue'
import ActionBar from './components/ActionBar.vue'

const team = useTeam()

onMounted(team.start)
onUnmounted(team.stop)
</script>

<template>
  <main
    v-if="team.cliProblem"
    class="flex h-screen flex-col items-center justify-center gap-4 bg-canvas p-8 text-center text-ink"
  >
    <h1 class="text-xl font-semibold">A Vokia precisa da CLI do Claude Code</h1>
    <p class="max-w-lg text-sm text-ink/70">{{ team.cliProblem.reason }}</p>
    <p class="text-sm text-ink/70">Para resolver, rode no terminal e reabra o app:</p>
    <code class="rounded-xl border border-line bg-white px-4 py-2 text-sm text-accent">
      {{ team.cliProblem.fixCommand }}
    </code>
  </main>
  <div v-else-if="team.cliChecked" class="flex h-screen bg-canvas text-ink">
    <AppRail />
    <AgentList />
    <Conversation />
    <ActionBar />
  </div>
</template>
