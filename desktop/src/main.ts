import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { useTheme } from './features/theme/useTheme'
import { useAvatarProfiles } from './features/identities/useAvatarProfiles'
import './styles/main.css'

async function bootstrap(): Promise<void> {
  const pinia = createPinia()
  await useTheme(pinia).load()
  await useAvatarProfiles(pinia).load()
  createApp(App).use(pinia).mount('#app')
}

void bootstrap()
