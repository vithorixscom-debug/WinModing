<script setup>
import { ref, watch } from 'vue'
import { LogOutIcon, LogInIcon, BoxIcon, FolderSearchIcon, TrashIcon, PirateShipIcon, UpdatedIcon } from '@modrinth/assets'
import { Card, Slider, DropdownSelect, Toggle, Button, Chips } from '@modrinth/ui'
import { handleError, useTheming } from '@/store/state'
import { get, set } from '@/helpers/settings'
import { get_java_versions, get_max_memory, set_java_version } from '@/helpers/jre'
import { get as getCreds, logout } from '@/helpers/mr_auth.js'
import JavaSelector from '@/components/ui/JavaSelector.vue'
import ModrinthLoginScreen from '@/components/ui/tutorial/ModrinthLoginScreen.vue'
import { open } from '@/helpers/dialog.js'
import { getOS } from '@/helpers/utils.js'
// import { getVersion } from '@tauri-apps/api/app'
import { get_user, purge_cache_types } from '@/helpers/cache.js'
import ConfirmModalWrapper from '@/components/ui/modal/ConfirmModalWrapper.vue'
import { useI18n } from '@/helpers/i18n.js'

import {
  getRemote,
  getBranches,
  launcherUrl,
  latestBetaCommitLink,
  latestBetaCommitTruncatedSha,
} from '@/helpers/update.js'
import { version } from '../../package.json'

const development_build = false

const pageOptions = ['Home', 'Library']
const catalogSourceOptions = ['modrinth', 'curseforge', 'both']
const catalogPageSizes = [5, 10, 15, 20, 50, 100]

const themeStore = useTheming()
const { t: i18n, locale, setLocale, localeOptions } = useI18n()
function catalogSourceLabel(item) {
  if (item === 'curseforge') return i18n.value('browse.sourceCurseforge')
  if (item === 'both') return i18n.value('browse.sourceBoth')
  return i18n.value('browse.sourceModrinth')
}

function duplicatePrefLabel(item) {
  if (item === 'curseforge') return i18n.value('browse.cardCurseforge')
  if (item === 'both') return i18n.value('browse.cardBoth')
  return i18n.value('browse.cardModrinth')
}

const accessSettings = async () => {
  const settings = await get()

  settings.launchArgs = (settings.extra_launch_args ?? []).join(' ')
  settings.envVars = (settings.custom_env_vars ?? [])
    .map((x) => (Array.isArray(x) ? x.join('=') : String(x)))
    .join(' ')

  return settings
}

const fetchSettings = await accessSettings().catch(handleError)

const settings = ref(fetchSettings ?? (await get()))

const maxMemory = ref(Math.floor(((await get_max_memory().catch(handleError)) || 33554432) / 1024))

watch(
  settings,
  async (oldSettings, newSettings) => {
    if (oldSettings.loaded_config_dir !== newSettings.loaded_config_dir) {
      return
    }

    const setSettings = JSON.parse(JSON.stringify(newSettings))

    setSettings.extra_launch_args = setSettings.launchArgs.trim().split(/\s+/).filter(Boolean)
    setSettings.custom_env_vars = setSettings.envVars
      .trim()
      .split(/\s+/)
      .filter(Boolean)
      .map((x) => x.split('=').filter(Boolean))

    if (!setSettings.hooks.pre_launch) {
      setSettings.hooks.pre_launch = null
    }
    if (!setSettings.hooks.wrapper) {
      setSettings.hooks.wrapper = null
    }
    if (!setSettings.hooks.post_exit) {
      setSettings.hooks.post_exit = null
    }

    if (!setSettings.custom_dir) {
      setSettings.custom_dir = null
    }

    await set(setSettings)
  },
  { deep: true },
)

const detectedJava = (await get_java_versions().catch(handleError)) || []
const detectedList = Array.isArray(detectedJava) ? detectedJava : Object.values(detectedJava)
function pickJava(major) {
  const saved = settings.value.java_globals?.[major] || settings.value.java_globals?.[String(major)]
  return saved || detectedList.find((item) => Number(item.major) === Number(major)) || { path: '', major }
}
const javaVersions = ref({
  8: pickJava(8),
  17: pickJava(17),
  21: pickJava(21),
})
async function updateJavaVersion(version) {
  if (version?.path === '') {
    version.path = undefined
  }

  if (version?.path) {
    version.path = version.path.replace('java.exe', 'javaw.exe')
  }

  if (version?.major != null) {
    javaVersions.value[version.major] = version
    settings.value.java_globals = {
      ...(settings.value.java_globals || {}),
      [String(version.major)]: { path: version.path },
    }
  }

  await set_java_version(version).catch(handleError)
}

async function fetchCredentials() {
  const creds = await getCreds().catch(handleError)
  if (creds && creds.user_id) {
    creds.user = await get_user(creds.user_id).catch(handleError)
  }
  credentials.value = creds
}

const credentials = ref()
await fetchCredentials()

const loginScreenModal = ref()

async function logOut() {
  await logout().catch(handleError)
  await fetchCredentials()
}

async function signInAfter() {
  await fetchCredentials()
}

async function findLauncherDir() {
  const newDir = await open({
    multiple: false,
    directory: true,
    title: 'Select a new app directory',
  })

  if (newDir) {
    settings.value.custom_dir = newDir
  }
}

async function purgeCache() {
  await purge_cache_types([
    'project',
    'version',
    'user',
    'team',
    'organization',
    'loader_manifest',
    'minecraft_manifest',
    'categories',
    'report_types',
    'loaders',
    'game_versions',
    'donation_platforms',
    'file_update',
    'search_results',
  ]).catch(handleError)
}

await getRemote(false, false)
await getBranches()

</script>

<template>
  <div class="settings-page">
    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.general') }}</span>
        </h3>
      </div>
      <div class="adjacent-input">
        <label for="language">
          <span class="label__title">{{ i18n('settings.language') }}</span>
          <span class="label__description">{{ i18n('settings.languageHint') }}</span>
        </label>
        <DropdownSelect
          id="language"
          name="Language"
          :options="localeOptions"
          :model-value="localeOptions.find((item) => item.id === locale)"
          :display-name="(item) => item?.label"
          @change="(e) => setLocale(e?.option?.id || e?.id || 'en')"
        />
      </div>
      <ConfirmModalWrapper ref="purgeCacheConfirmModal" :title="i18n('settings.purgeTitle')"
        :description="i18n('settings.purgeBody')"
        :has-to-type="false" :proceed-label="i18n('settings.purgeCache')" @proceed="purgeCache" />
      <div class="adjacent-input">
        <label for="purge-cache">
          <span class="label__title">{{ i18n('settings.cacheTitle') }}</span>
          <span class="label__description">
            {{ i18n('settings.cache') }}
          </span>
        </label>
        <button id="purge-cache" class="btn" @click="$refs.purgeCacheConfirmModal.show()">
          <TrashIcon />
          {{ i18n('settings.purgeCache') }}
        </button>
      </div>
    </Card>
    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.resources') }}</span>
        </h3>
      </div>
      <label for="appDir">
        <span class="label__title">{{ i18n('settings.appDir') }}</span>
        <span class="label__description">
          {{ i18n('settings.appDirHint') }}
        </span>
      </label>
      <div class="app-directory">
        <div class="iconified-input">
          <BoxIcon />
          <input
            id="appDir"
            v-model="settings.custom_dir"
            type="text"
            class="input"
            :placeholder="settings.loaded_config_dir || ''"
          />
          <Button class="r-btn" @click="findLauncherDir">
            <FolderSearchIcon />
          </Button>
        </div>
      </div>
    </Card>
    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.display') }}</span>
        </h3>
      </div>
      <div class="adjacent-input">
        <label for="theme">
          <span class="label__title">{{ i18n('settings.theme') }}</span>
          <span class="label__description">{{ i18n('settings.themeHint') }}</span>
        </label>
        <DropdownSelect id="theme" name="Theme dropdown" :options="themeStore.themeOptions"
          :default-value="settings.theme" :model-value="settings.theme" class="theme-dropdown" @change="(e) => {
              themeStore.setThemeState(e.option.toLowerCase())
              settings.theme = themeStore.selectedTheme
            }
            " />
      </div>
      <div class="adjacent-input">
        <label for="advanced-rendering">
          <span class="label__title">{{ i18n('settings.advancedRendering') }}</span>
          <span class="label__description">{{ i18n('settings.advancedRenderingHint') }}</span>
        </label>
        <Toggle id="advanced-rendering" :model-value="themeStore.advancedRendering"
          :checked="themeStore.advancedRendering" @update:model-value="(e) => {
              themeStore.advancedRendering = e
              settings.advanced_rendering = themeStore.advancedRendering
            }
            " />
      </div>
      <div class="adjacent-input">
        <label for="minimize-launcher">
          <span class="label__title">{{ i18n('settings.minimize') }}</span>
          <span class="label__description">{{ i18n('settings.minimizeHint') }}</span>
        </label>
        <Toggle id="minimize-launcher" :model-value="settings.hide_on_process_start"
          :checked="settings.hide_on_process_start" @update:model-value="(e) => {
              settings.hide_on_process_start = e
            }
            " />
      </div>
      <div class="adjacent-input">
        <label for="disable-discord-rpc">
          <span class="label__title">{{ i18n('settings.discord') }}</span>
          <span class="label__description">{{ i18n('settings.discordHint') }}</span>
        </label>
        <Toggle id="disable-discord-rpc" v-model="settings.discord_rpc" :checked="settings.discord_rpc" />
      </div>
      <div v-if="getOS() != 'MacOS'" class="adjacent-input">
        <label for="native-decorations">
          <span class="label__title">{{ i18n('settings.native') }}</span>
          <span class="label__description">{{ i18n('settings.nativeHint') }}</span>
        </label>
        <Toggle id="native-decorations" :model-value="settings.native_decorations"
          :checked="settings.native_decorations" @update:model-value="(e) => {
              settings.native_decorations = e
            }
            " />
      </div>
      <div class="adjacent-input">
        <label for="opening-page">
          <span class="label__title">{{ i18n('settings.landing') }}</span>
          <span class="label__description">{{ i18n('settings.landingHint') }}</span>
        </label>
        <DropdownSelect id="opening-page" name="Opening page dropdown" :options="pageOptions"
          :default-value="settings.default_page" :model-value="settings.default_page" class="opening-page" @change="(e) => {
              settings.default_page = e.option
            }
            " />
      </div>
    </Card>
    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.catalog') }}</span>
        </h3>
      </div>
      <div class="adjacent-input catalog-setting">
        <label>
          <span class="label__title">{{ i18n('settings.catalogSource') }}</span>
          <span class="label__description">{{ i18n('settings.catalogSourceHint') }}</span>
        </label>
        <Chips
          v-model="settings.catalog_source"
          :items="catalogSourceOptions"
          :capitalize="false"
          :format-label="catalogSourceLabel"
        />
      </div>
      <div v-if="settings.catalog_source === 'both'" class="adjacent-input catalog-setting">
        <label>
          <span class="label__title">{{ i18n('settings.duplicatePref') }}</span>
          <span class="label__description">{{ i18n('settings.duplicatePrefHint') }}</span>
        </label>
        <Chips
          v-model="settings.catalog_duplicate_pref"
          :items="catalogSourceOptions"
          :capitalize="false"
          :format-label="duplicatePrefLabel"
        />
      </div>
      <div class="adjacent-input catalog-setting">
        <label for="catalog-page-size">
          <span class="label__title">{{ i18n('settings.catalogPageSize') }}</span>
          <span class="label__description">{{ i18n('settings.catalogPageSizeHint') }}</span>
        </label>
        <Chips
          :model-value="String(settings.catalog_page_size)"
          :items="catalogPageSizes.map(String)"
          :capitalize="false"
          @update:model-value="settings.catalog_page_size = Number($event) || 20"
        />
      </div>
    </Card>
    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.resources') }}</span>
        </h3>
      </div>

      <div class="adjacent-input">
        <label for="max-downloads">
          <span class="label__title">{{ i18n('settings.maxDownloads') }}</span>
          <span class="label__description">{{ i18n('settings.maxDownloadsHint') }}</span>
        </label>
        <Slider id="max-downloads" v-model="settings.max_concurrent_downloads" :min="1" :max="10" :step="1" />
      </div>

      <div class="adjacent-input">
        <label for="max-writes">
          <span class="label__title">{{ i18n('settings.maxWrites') }}</span>
          <span class="label__description">{{ i18n('settings.maxWritesHint') }}</span>
        </label>
        <Slider id="max-writes" v-model="settings.max_concurrent_writes" :min="1" :max="50" :step="1" />
      </div>
    </Card>
    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.privacy') }}</span>
        </h3>
      </div>
      <p class="label__description">{{ i18n('settings.privacyBody') }}</p>
    </Card>
    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.java') }}</span>
        </h3>
      </div>
      <template v-for="javaVersion in [21, 17, 8]" :key="`java-${javaVersion}`">
        <label :for="'java-' + javaVersion">
          <span class="label__title">{{ i18n('settings.javaLocation', { n: javaVersion }) }}</span>
        </label>
        <JavaSelector
          :id="'java-selector-' + javaVersion"
          v-model="javaVersions[javaVersion]"
          :version="javaVersion"
          @update:model-value="(e) => updateJavaVersion({ ...e, major: javaVersion })"
        />
      </template>
      <hr class="card-divider" />
      <label for="java-args">
        <span class="label__title">{{ i18n('settings.javaArgs') }}</span>
      </label>
      <input id="java-args" v-model="settings.launchArgs" autocomplete="off" type="text" class="installation-input"
        placeholder="Enter java arguments..." />
      <label for="env-vars">
        <span class="label__title">{{ i18n('settings.envVars') }}</span>
      </label>
      <input id="env-vars" v-model="settings.envVars" autocomplete="off" type="text" class="installation-input"
        placeholder="Enter environmental variables..." />
      <hr class="card-divider" />
      <div class="adjacent-input">
        <label for="max-memory">
          <span class="label__title">{{ i18n('settings.javaMemory') }}</span>
          <span class="label__description">{{ i18n('settings.javaMemoryHint') }}</span>
        </label>
        <Slider id="max-memory" v-model="settings.memory.maximum" :min="8" :max="maxMemory" :step="64" unit="mb" />
      </div>
    </Card>
    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.hooks') }}</span>
        </h3>
      </div>
      <div class="adjacent-input">
        <label for="pre-launch">
          <span class="label__title">{{ i18n('settings.preLaunch') }}</span>
          <span class="label__description">{{ i18n('settings.preLaunchHint') }}</span>
        </label>
        <input id="pre-launch" v-model="settings.hooks.pre_launch" autocomplete="off" type="text"
          placeholder="Enter pre-launch command..." />
      </div>
      <div class="adjacent-input">
        <label for="wrapper">
          <span class="label__title">{{ i18n('settings.wrapper') }}</span>
          <span class="label__description">{{ i18n('settings.wrapperHint') }}</span>
        </label>
        <input id="wrapper" v-model="settings.hooks.wrapper" autocomplete="off" type="text"
          placeholder="Enter wrapper command..." />
      </div>
      <div class="adjacent-input">
        <label for="post-exit">
          <span class="label__title">{{ i18n('settings.postExit') }}</span>
          <span class="label__description">{{ i18n('settings.postExitHint') }}</span>
        </label>
        <input id="post-exit" v-model="settings.hooks.post_exit" autocomplete="off" type="text"
          placeholder="Enter post-exit command..." />
      </div>
    </Card>
    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.window') }}</span>
        </h3>
      </div>
      <div class="adjacent-input">
        <label for="fullscreen">
          <span class="label__title">{{ i18n('settings.fullscreen') }}</span>
          <span class="label__description">{{ i18n('settings.fullscreenHint') }}</span>
        </label>
        <Toggle id="fullscreen" :model-value="settings.force_fullscreen" :checked="settings.force_fullscreen"
          @update:model-value="(e) => {
              settings.force_fullscreen = e
            }
            " />
      </div>
      <div class="adjacent-input">
        <label for="width">
          <span class="label__title">{{ i18n('settings.width') }}</span>
          <span class="label__description">{{ i18n('settings.widthHint') }}</span>
        </label>
        <input id="width" v-model="settings.game_resolution[0]" :disabled="settings.force_fullscreen" autocomplete="off"
          type="number" placeholder="Enter width..." />
      </div>
      <div class="adjacent-input">
        <label for="height">
          <span class="label__title">{{ i18n('settings.height') }}</span>
          <span class="label__description">{{ i18n('settings.heightHint') }}</span>
        </label>
        <input id="height" v-model="settings.game_resolution[1]" :disabled="settings.force_fullscreen"
          autocomplete="off" type="number" class="input" placeholder="Enter height..." />
      </div>
    </Card>
    <Card>
      <div class="label inline-fix">
        <h3>
          <span class="label__title size-card-header in"
            > {{ i18n('settings.aboutHeader') }}
            <p v-if="development_build" class="development option">
              {{ i18n('settings.devWarn') }}
            </p>
          </span>
        </h3>
      </div>
      <div>
        <label>
          <span class="label__title inl">{{ i18n('settings.about') }} • {{ version }}</span>

          <span class="label__description"
            >Latest beta commit • 
            <a class="github" :href="latestBetaCommitLink">{{
              latestBetaCommitTruncatedSha
            }}</a></span
          >
          <span class="label__description"
            >All latest versions always published on GitHub
            <a class="github" :href="launcherUrl">Our GitHub repository</a></span
          >

          <span class="label__title">Update Checker</span>

          <span class="label__description"
            >Version on remote server • 
            <p id="releaseData" class="cosmic inline-fix"></p>
          </span>
          <span class="label__description"
            >Version on local device • 
            <p class="cosmic inline-fix">v{{ version }}</p></span
          >
        </label>
        <div class="inline-item-group">
          <Button icon-only @click="getRemote(false, false), getBranches()">
            <UpdatedIcon /> {{ i18n('settings.checkUpdates') }}
          </Button>
        </div>
      </div>
    </Card>
  </div>
</template>

<style lang="scss" scoped>
.settings-page {
  margin: 1rem;
}

.installation-input {
  width: 100% !important;
  flex-grow: 1;
}

.theme-dropdown {
  text-transform: capitalize;
}

.catalog-setting {
  flex-wrap: wrap;
  align-items: flex-start;
  gap: 0.85rem;
  padding: 0.35rem 0;

  :deep(.chips) {
    flex-wrap: wrap;
    gap: 0.55rem;
  }

  :deep(.chips .btn) {
    white-space: nowrap;
    padding: 0.55rem 0.95rem;
  }
}

.card-divider {
  margin: 1rem 0;
}

.app-directory {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: var(--gap-sm);

  .iconified-input {
    flex-grow: 1;

    input {
      flex-basis: auto;
    }
  }
}

.development {
  color: #ff6a00;
  text-decoration: none;
  text-shadow:
    0 0 4px rgba(79, 173, 255, 0.5),
    0 0 8px rgba(14, 98, 204, 0.5),
    0 0 12px rgba(122, 31, 199, 0.5);
  transition: color 1.5s ease;
}
.development:hover,
.development:focus,
.development:active {
  color: #4800d3;
  text-shadow: #801313;
}

.cosmic {
  color: #3e8cde;
  text-decoration: none;
  text-shadow:
    0 0 4px rgba(79, 173, 255, 0.5),
    0 0 8px rgba(14, 98, 204, 0.5),
    0 0 12px rgba(122, 31, 199, 0.5);
  transition: color 0.35s ease;
}
.cosmic:hover,
.cosmic:focus,
.cosmic:active {
  color: #10fae5;
  text-shadow: #26065e;
}

.download {
  color: #3e8cde;
  border: none;
  padding: var(--gap-sm) var(--gap-lg);
  //background-color: rgba(0, 0, 0, 0.0);
  text-decoration: none;
  text-shadow:
    0 0 4px rgba(79, 173, 255, 0.5),
    0 0 8px rgba(14, 98, 204, 0.5),
    0 0 12px rgba(122, 31, 199, 0.5);
  transition: color 0.35s ease;
}
.download:hover,
.download:focus,
.download:active {
  color: #10fae5;
  text-shadow: #26065e;
}

a.github {
  color: #3e8cde;
  text-decoration: none;
  text-shadow:
    0 0 4px rgba(79, 173, 255, 0.5),
    0 0 8px rgba(14, 98, 204, 0.5),
    0 0 12px rgba(122, 31, 199, 0.5);
  transition: color 0.35s ease;
}

a.github:hover,
a.github:focus,
a.github:active {
  color: #10fae5;
  text-shadow: #26065e;
}

.inline-item-group {
  display: inline-flex;
  gap: 0.25rem;
}

.inline-fix {
  display: inline-flex;
  margin-top: -2rem;
  margin-bottom: -2rem;
}

.download-modal {
  color: #3e8cde;
  padding: var(--gap-sm) var(--gap-lg);
  text-decoration: none;
  text-shadow:
    0 0 4px rgba(79, 173, 255, 0.5),
    0 0 8px rgba(14, 98, 204, 0.5),
    0 0 12px rgba(122, 31, 199, 0.5);
  transition: color 0.35s ease;
}
.download-modal:hover,
.download-modal:focus,
.download-modal:active {
  color: #10fae5;
  text-shadow: #26065e;
}

.option {
  background: var(--color-bg);
  border-radius: var(--radius-lg);
  width: auto;
  display: inline-flex;
  align-items: center;
  margin-top: auto;
  margin-left: 0.5rem;
  font-size: 1rem;
  padding: 0.5rem;
}
</style>
