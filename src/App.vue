<script setup>
import { computed, ref, onMounted } from 'vue'
import { RouterView, RouterLink, useRouter, useRoute } from 'vue-router'
import {
  HomeIcon,
  SearchIcon,
  LibraryIcon,
  PlusIcon,
  SettingsIcon,
  XIcon,
  MinimizeIcon,
  MaximizeIcon,
  RestoreIcon,
  UserIcon,
  ServerIcon,
} from '@modrinth/assets'
import { Button, Notifications } from '@modrinth/ui'
import { useLoading, useTheming } from '@/store/state'
import AccountsCard from '@/components/ui/AccountsCard.vue'
import InstanceCreationModal from '@/components/ui/InstanceCreationModal.vue'
import { get } from '@/helpers/settings'
import Breadcrumbs from '@/components/ui/Breadcrumbs.vue'
import RunningAppBar from '@/components/ui/RunningAppBar.vue'
import SplashScreen from '@/components/ui/SplashScreen.vue'
import ErrorModal from '@/components/ui/ErrorModal.vue'
import ModrinthLoadingIndicator from '@/components/modrinth-loading-indicator'
import { handleError, useNotifications } from '@/store/notifications.js'
import { command_listener, warning_listener, process_listener } from '@/helpers/events.js'
import QuickInstanceSwitcher from '@/components/ui/QuickInstanceSwitcher.vue'
import { useI18n } from '@/helpers/i18n.js'
import { isDev, getOS, openUrl } from '@/helpers/utils.js'
import { getCurrentWindow } from '@tauri-apps/api/window'
import URLConfirmModal from '@/components/ui/URLConfirmModal.vue'
import { install_from_file } from './helpers/pack'
import { useError } from '@/store/error.js'
import { useCheckDisableMouseover } from '@/composables/macCssFix.js'
import ModInstallModal from '@/components/ui/install_flow/ModInstallModal.vue'
import IncompatibilityWarningModal from '@/components/ui/install_flow/IncompatibilityWarningModal.vue'
import InstallConfirmModal from '@/components/ui/install_flow/InstallConfirmModal.vue'
import { useInstall } from '@/store/install.js'
import { invoke } from '@tauri-apps/api/core'
import { get_opening_command, initialize_state } from '@/helpers/state'
import { renderString } from '@modrinth/utils'

const themeStore = useTheming()
const router = useRouter()
const route = useRoute()
const loading = useLoading()
const notifications = useNotifications()
const error = useError()
const install = useInstall()
const { t: i18n, locale } = useI18n()

const urlModal = ref(null)
const notificationsWrapper = ref()
const errorModal = ref()
const modInstallModal = ref()
const installConfirmModal = ref()
const incompatibilityWarningModal = ref()

const offline = ref(!navigator.onLine)
window.addEventListener('offline', () => {
  offline.value = true
})
window.addEventListener('online', () => {
  offline.value = false
})

const showOnboarding = ref(false)
const nativeDecorations = ref(false)

const os = ref('')

const stateInitialized = ref(false)

const criticalErrorMessage = ref()
const isMaximized = ref(false)

onMounted(async () => {
  await useCheckDisableMouseover()
  try {
    isMaximized.value = await getCurrentWindow().isMaximized()
    await getCurrentWindow().onResized(async () => {
      isMaximized.value = await getCurrentWindow().isMaximized()
    })
  } catch {
    /* web / demo */
  }
})

async function setupApp() {
  stateInitialized.value = true
  const {
    native_decorations,
    theme,
    collapsed_navigation,
    advanced_rendering,
    onboarded,
    default_page,
  } = await get()

  if (default_page && default_page !== 'Home') {
    await router.push({ name: default_page })
  }

  os.value = await getOS().catch(() => 'Windows')
  const dev = await isDev().catch(() => true)
  showOnboarding.value = !onboarded

  nativeDecorations.value = native_decorations
  try {
    if (os.value !== 'MacOS') await getCurrentWindow().setDecorations(native_decorations)
  } catch {
    /* ignore */
  }

  themeStore.setThemeState(theme)
  themeStore.collapsedNavigation = collapsed_navigation
  themeStore.advancedRendering = advanced_rendering

  if (!dev) document.addEventListener('contextmenu', (event) => event.preventDefault())

  document.documentElement.classList.add('windows')

  try {
    await warning_listener((e) =>
      notificationsWrapper.value.addNotification({
        title: 'Warning',
        text: e.message,
        type: 'warn',
      }),
    )
  } catch {
    /* ignore */
  }

  get_opening_command().then(handleCommand)
}

const stateFailed = ref(false)
initialize_state()
  .then(() => {
    setupApp().catch((err) => {
      stateFailed.value = true
      console.error(err)
      error.showError(err, null, false, 'state_init')
    })
  })
  .catch((err) => {
    stateFailed.value = true
    console.error('Failed to initialize app', err)
    error.showError(err, null, false, 'state_init')
  })

const handleClose = async () => {
  try {
    await getCurrentWindow().close()
  } catch {
    window.close()
  }
}

function beginDrag(event) {
  if (event.button !== 0) return
  const block = event.target?.closest?.(
    'button, a, input, textarea, select, .no-drag, .window-controls, .titlebar-button',
  )
  if (block) return
  // One drag path only — Windows uses native drag_window; fallback to Tauri API.
  invoke('drag_window').catch(() => {
    getCurrentWindow()
      .startDragging()
      .catch(() => {})
  })
}

const isOnBrowse = computed(() => route.path.startsWith('/browse'))

loading.setEnabled(false)

onMounted(() => {
  invoke('show_window')

  notifications.setNotifs(notificationsWrapper.value)

  error.setErrorModal(errorModal.value)

  install.setIncompatibilityWarningModal(incompatibilityWarningModal)
  install.setInstallConfirmModal(installConfirmModal)
  install.setModInstallModal(modInstallModal)
})

document.querySelector('body').addEventListener('click', function (e) {
  let target = e.target
  while (target != null) {
    if (target.matches?.('a')) {
      const href = target.href
      if (
        href &&
        ['http://', 'https://', 'mailto:', 'tel:'].some((v) => href.startsWith(v)) &&
        !target.classList.contains('router-link-active') &&
        !href.startsWith('http://localhost') &&
        !href.startsWith('https://tauri.localhost') &&
        !href.startsWith('http://tauri.localhost')
      ) {
        e.preventDefault()
        openUrl(href)
      }
      break
    }
    target = target.parentElement
  }
})

document.querySelector('body').addEventListener('auxclick', function (e) {
  // disables middle click -> new tab
  if (e.button === 1) {
    e.preventDefault()
    // instead do a left click
    const event = new MouseEvent('click', {
      view: window,
      bubbles: true,
      cancelable: true,
    })
    e.target.dispatchEvent(event)
  }
})

const accounts = ref(null)

command_listener(handleCommand)
process_listener((e) => {
  const event = String(e?.event || '').toLowerCase()
  if (event === 'finished') {
    invoke('discord_set_idle').catch(() => {})
  }
})
async function handleCommand(e) {
  if (!e) return

  if (e.event === 'RunMRPack') {
    // RunMRPack should directly install a local mrpack given a path
    if (e.path.endsWith('.mrpack')) {
      await install_from_file(e.path).catch(handleError)
    }
  } else {
    // Other commands are URL-based (deep linking)
    urlModal.value.show(e)
  }
}

// const updateAvailable = ref(false)
// async function checkUpdates() {
//   const update = await check()
//   console.log(update)
//   updateAvailable.value = !!update

//   setTimeout(
//     () => {
//       checkUpdates()
//     },
//     5 * 1000 * 60,
//   )
// }
</script>

<template>
  <SplashScreen v-if="!stateFailed" ref="splashScreen" data-tauri-drag-region />
  <div v-if="stateInitialized" class="app-container">
    <div class="nav-container">
      <div class="nav-section">
        <AccountsCard ref="accounts" mode="small" />
        <div class="pages-list">
          <RouterLink v-tooltip="i18n('nav.home')" to="/" class="btn icon-only collapsed-button">
            <HomeIcon />
          </RouterLink>
          <RouterLink
            v-tooltip="i18n('nav.browse')"
            to="/browse/mod"
            class="btn icon-only collapsed-button"
            :class="{
              'router-link-active': isOnBrowse,
            }"
          >
            <SearchIcon />
          </RouterLink>
          <RouterLink v-tooltip="i18n('nav.library')" to="/library" class="btn icon-only collapsed-button">
            <LibraryIcon />
          </RouterLink>
          <RouterLink v-tooltip="i18n('nav.skins')" to="/skins" class="btn icon-only collapsed-button">
            <UserIcon />
          </RouterLink>
          <RouterLink v-tooltip="i18n('nav.servers')" to="/servers" class="btn icon-only collapsed-button">
            <ServerIcon />
          </RouterLink>
          <Suspense>
            <QuickInstanceSwitcher />
          </Suspense>
          <Suspense>
            <InstanceCreationModal ref="installationModal" />
          </Suspense>
        </div>
      </div>
      <div class="settings pages-list">
        <Button
          v-tooltip="i18n('nav.create')"
          class="sleek-primary collapsed-button"
          icon-only
          :disabled="offline"
          @click="() => $refs.installationModal.show()"
        >
          <PlusIcon />
        </Button>
        <RouterLink v-tooltip="i18n('nav.settings')" to="/settings" class="btn icon-only collapsed-button">
          <SettingsIcon />
        </RouterLink>
      </div>
    </div>
    <div class="view">
      <div v-if="criticalErrorMessage" class="critical-error-banner" data-tauri-drag-region>
        <h1>{{ criticalErrorMessage.header }}</h1>
        <div class="markdown-body" v-html="renderString(criticalErrorMessage.body ?? '')"></div>
      </div>
      <div class="appbar-row">
        <div class="appbar" @mousedown="beginDrag">
          <section class="navigation-controls">
            <Breadcrumbs data-tauri-drag-region />
          </section>
          <section class="mod-stats">
            <Suspense>
              <RunningAppBar />
            </Suspense>
          </section>
        </div>
        <section v-if="!nativeDecorations" class="window-controls no-drag">
          <Button class="titlebar-button" icon-only @click="() => getCurrentWindow().minimize()">
            <MinimizeIcon />
          </Button>
          <Button
            class="titlebar-button"
            icon-only
            @click="() => getCurrentWindow().toggleMaximize()"
          >
            <RestoreIcon v-if="isMaximized" />
            <MaximizeIcon v-else />
          </Button>
          <Button class="titlebar-button close" icon-only @click="handleClose">
            <XIcon />
          </Button>
        </section>
      </div>
      <div class="router-view">
        <ModrinthLoadingIndicator
          offset-height="var(--appbar-height)"
          offset-width="var(--sidebar-width)"
        />
        <RouterView v-slot="{ Component }">
          <template v-if="Component">
            <Suspense @pending="loading.startLoading()" @resolve="loading.stopLoading()">
              <component
                :is="Component"
                :key="`${locale}:${route.params.id || route.path}`"
              ></component>
            </Suspense>
          </template>
        </RouterView>
      </div>
    </div>
  </div>
  <URLConfirmModal ref="urlModal" />
  <Notifications ref="notificationsWrapper" />
  <ErrorModal ref="errorModal" />
  <ModInstallModal ref="modInstallModal" />
  <IncompatibilityWarningModal ref="incompatibilityWarningModal" />
  <InstallConfirmModal ref="installConfirmModal" />
</template>

<style lang="scss" scoped>
.sleek-primary {
  background-color: var(--color-brand-highlight);
  transition: all ease-in-out 0.1s;
}

.navigation-controls {
  flex-grow: 1;
  width: min-content;
  min-width: 0;
}

.appbar-row {
  position: relative;
  z-index: 8;
  display: flex;
  flex-direction: row;
  min-height: 3.25rem;
}

.window-controls {
  z-index: 20;
  display: none;
  flex-direction: row;
  align-items: center;
  gap: 0.45rem;
  padding-right: 0.5rem;

  .titlebar-button {
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all ease-in-out 0.1s;
    background-color: transparent !important;
    color: var(--color-base);
    height: 3.25rem;
    width: 3.15rem;
    min-width: 3.15rem;
    border-radius: 0 !important;
    position: relative;
    box-shadow: none !important;

    svg {
      width: 1.25rem;
      height: 1.25rem;
    }

    &::before {
      content: '';
      border-radius: 999999px;
      width: 2.75rem;
      height: 2.75rem;
      aspect-ratio: 1 / 1;
      margin-block: auto;
      position: absolute;
      left: 50%;
      top: 50%;
      translate: -50% -50%;
      background-color: transparent;
      scale: 0.88;
      transition: all ease-in-out 0.2s;
      z-index: -1;
    }

    &.close {
      &::before {
        width: 3rem;
        height: 3rem;
      }

      &:hover,
      &:active {
        color: var(--color-accent-contrast);

        &::before {
          background-color: var(--color-red);
          scale: 1;
        }
      }
    }

    &:hover,
    &:active {
      color: var(--color-contrast);

      &::before {
        background-color: var(--color-button-bg);
        scale: 1;
      }
    }
  }
}

.app-container {
  --appbar-height: 3.25rem;
  --sidebar-width: 4.5rem;

  height: 100vh;
  display: flex;
  flex-direction: row;
  overflow: hidden;

  .view {
    width: calc(100% - var(--sidebar-width));
    background-color: var(--color-raised-bg);

    .critical-error-banner {
      margin-top: -1.25rem;
      padding: 1rem;
      background-color: rgba(203, 34, 69, 0.1);
      border-left: 2px solid var(--color-red);
      border-bottom: 2px solid var(--color-red);
      border-right: 2px solid var(--color-red);
      border-radius: 1rem;
    }

    .appbar {
      display: flex;
      align-items: center;
      flex-grow: 1;
      background: var(--color-raised-bg);
      text-align: center;
      padding: var(--gap-md);
      height: 3.25rem;
      gap: var(--gap-sm);
      //no select
      user-select: none;
      -webkit-user-select: none;
    }

    .router-view {
      width: 100%;
      height: calc(100% - 3.125rem);
      overflow: auto;
      overflow-x: hidden;
      background-color: var(--color-bg);
      border-top-left-radius: var(--radius-xl);
    }
  }
}

.nav-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: space-between;
  width: var(--sidebar-width);
  height: 100%;
  background-color: var(--color-raised-bg);
  box-shadow: var(--shadow-inset-sm), var(--shadow-floating);
  padding: 0.85rem 0;
}

.pages-list {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  width: 100%;
  gap: 0.5rem;

  a {
    display: flex;
    align-items: center;
    word-spacing: 3px;
    background: inherit;
    transition: all ease-in-out 0.1s;
    color: var(--color-base);
    box-shadow: none;

    &.router-link-active {
      color: var(--color-contrast);
      background: var(--color-button-bg);
      box-shadow: var(--shadow-floating);
    }

    &:hover {
      background-color: var(--color-button-bg);
      color: var(--color-contrast);
      box-shadow: 0px 4px 4px rgba(0, 0, 0, 0.25);
      text-decoration: none;
    }
  }

  &.primary {
    color: var(--color-accent-contrast);
    background-color: var(--color-brand);
  }
}

.collapsed-button {
  height: 3rem !important;
  width: 3rem !important;
  min-width: 3rem;
  min-height: 3rem;
  padding: 0.75rem !important;
  border-radius: var(--radius-md);
  box-shadow: none;
  display: flex !important;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  margin-inline: auto;

  svg {
    width: 1.5rem !important;
    height: 1.5rem !important;
    max-width: 1.5rem !important;
    max-height: 1.5rem !important;
  }
}

.nav-section {
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  align-items: center;
  width: 100%;
  flex: 1;
  min-height: 0;
  gap: 0.45rem;
}

.button-row {
  display: flex;
  flex-direction: row;
  justify-content: space-between;
  gap: var(--gap-md);

  .transparent {
    padding: var(--gap-sm) 0;
  }
}
</style>
<style>
.mac {
  .nav-container {
    padding-top: calc(var(--gap-md) + 1.75rem);
  }

  .account-card,
  .card-section {
    top: calc(var(--gap-md) + 1.75rem);
  }
}

.windows {
  .fake-appbar {
    height: 2.5rem !important;
  }

  .window-controls {
    display: flex !important;
  }

  .info-card {
    right: 8rem;
  }

  .profile-card {
    right: 8rem;
  }
}
</style>
<style src="vue-multiselect/dist/vue-multiselect.css"></style>
