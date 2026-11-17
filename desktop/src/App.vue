<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useConnection } from './features/team/useConnection'
import AppRail from './shell/AppRail.vue'
import { PAGE_LABELS, RAIL_COLLAPSED_WIDTH, RAIL_EXPANDED_WIDTH } from './shell/railNav'
import { useNavigation } from './shell/useNavigation'
import { useRail } from './shell/useRail'
import { PAGE_VIEWS } from './shell/pages'
import ActionBar from './features/team/ActionBar.vue'
import NewRequestDialog from './features/backlog/NewRequestDialog.vue'
import { useBacklog } from './features/backlog/useBacklog'
import UserProfileDialog from './features/profile/UserProfileDialog.vue'
import UpdatePrompt from './features/updater/UpdatePrompt.vue'
import { useUpdater } from './features/updater/useUpdater'
import { useUsage } from './features/usage/useUsage'
import { useLimitNotices } from './features/usage/useLimitNotices'
import SplashScreen from './features/system/SplashScreen.vue'
import CliMissingScreen from './features/system/CliMissingScreen.vue'
import UiEmpty from './ui/UiEmpty.vue'

const UNBUILT_PAGE_TEXT = 'Página ainda sem tela no app.'

const connection = useConnection()
const navigation = useNavigation()
const rail = useRail()
const backlog = useBacklog()
const updater = useUpdater()
const usage = useUsage()
const limitNotices = useLimitNotices()
const pageViews = computed(() => PAGE_VIEWS[navigation.page] ?? null)
const showRail = computed(() => navigation.page !== 'onboarding')
const railWidth = computed(() => {
  if (!showRail.value) return 0
  return rail.expanded ? RAIL_EXPANDED_WIDTH : RAIL_COLLAPSED_WIDTH
})

onMounted(connection.start)
onMounted(updater.check)
onMounted(usage.start)
onMounted(limitNotices.start)
onUnmounted(connection.stop)
onUnmounted(usage.stop)
onUnmounted(limitNotices.stop)
</script>

<template>
  <main
    v-if="connection.startupError"
    class="flex h-screen items-center justify-center bg-bg p-8 text-center text-body text-danger"
  >
    {{ connection.startupError }}
  </main>
  <CliMissingScreen
    v-else-if="connection.cliProblem"
    :problem="connection.cliProblem"
    @recheck="connection.recheckCli"
  />
  <div v-else-if="connection.cliChecked" class="flex h-screen bg-bg text-ink" :style="{ '--rail-w': `${railWidth}px` }">
    <AppRail v-if="showRail" />
    <template v-if="pageViews">
      <component :is="view" v-for="(view, index) in pageViews" :key="index" />
    </template>
    <UiEmpty
      v-else
      class="flex-1 justify-center"
      :title="PAGE_LABELS[navigation.page]"
      :text="UNBUILT_PAGE_TEXT"
    />
    <ActionBar @compose="backlog.startRequest()" />
  </div>
  <SplashScreen v-else />
  <NewRequestDialog />
  <UserProfileDialog v-if="connection.cliChecked" />
  <UpdatePrompt />
</template>
