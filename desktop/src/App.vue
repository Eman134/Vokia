<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useTeam } from './features/team/useTeam'
import AppRail from './shell/AppRail.vue'
import { PAGE_LABELS, RAIL_COLLAPSED_WIDTH } from './shell/railNav'
import { useNavigation } from './shell/useNavigation'
import { PAGE_VIEWS } from './shell/pages'
import ActionBar from './features/team/ActionBar.vue'
import UpdatePrompt from './features/updater/UpdatePrompt.vue'
import { useUpdater } from './features/updater/useUpdater'
import { useUsage } from './features/usage/useUsage'
import SplashScreen from './features/system/SplashScreen.vue'
import CliMissingScreen from './features/system/CliMissingScreen.vue'
import UiEmpty from './ui/UiEmpty.vue'

const UNBUILT_PAGE_TEXT = 'Página ainda sem tela no app.'

const team = useTeam()
const navigation = useNavigation()
const updater = useUpdater()
const usage = useUsage()
const pageViews = computed(() => PAGE_VIEWS[navigation.page] ?? null)
const showRail = computed(() => navigation.page !== 'onboarding')

onMounted(team.start)
onMounted(updater.check)
onMounted(usage.start)
onUnmounted(team.stop)
onUnmounted(usage.stop)
</script>

<template>
  <main
    v-if="team.startupError"
    class="flex h-screen items-center justify-center bg-bg p-8 text-center text-body text-danger"
  >
    {{ team.startupError }}
  </main>
  <CliMissingScreen
    v-else-if="team.cliProblem"
    :problem="team.cliProblem"
    @recheck="team.recheckCli"
  />
  <div v-else-if="team.cliChecked" class="flex h-screen bg-bg text-ink" :style="{ '--rail-w': `${showRail ? RAIL_COLLAPSED_WIDTH : 0}px` }">
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
    <ActionBar />
  </div>
  <SplashScreen v-else />
  <UpdatePrompt />
</template>
