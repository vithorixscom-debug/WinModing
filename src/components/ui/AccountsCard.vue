<template>
  <!-- Single root so sidebar gap is not inflated by modal placeholders. -->
  <div class="accounts-shell">
  <div
    v-if="mode !== 'isolated'"
    ref="button"
    v-tooltip.right="i18n('accounts.tooltip')"
    class="button-base avatar-button"
    :class="{ expanded: mode === 'expanded' }"
    @click="toggleMenu"
  >
    <img class="face" :src="selectedFace" alt="" width="48" height="48" draggable="false" />
  </div>

  <transition name="fade">
    <Card
      v-if="showCard || mode === 'isolated'"
      ref="card"
      class="account-card"
      :class="{ expanded: mode === 'expanded', isolated: mode === 'isolated' }"
    >
      <div v-if="selectedAccount" class="selected account">
        <img class="face face-md" :src="selectedFace" alt="" width="40" height="40" draggable="false" />
        <div class="selected-meta">
          <h4>{{ selectedAccount.username }}</h4>
          <p>
            <span class="kind-badge" :data-kind="accountKind(selectedAccount)">
              <SSOMicrosoftIcon v-if="accountKind(selectedAccount) === 'microsoft'" class="kind-svg" />
              <GlobeIcon v-else-if="accountKind(selectedAccount) === 'ely'" class="kind-svg" />
              <PirateIcon v-else class="kind-svg" />
            </span>
            {{ accountKindLabel(selectedAccount) }}
            <span v-if="selectedAccount.sessionOk" class="session-ok">{{ i18n('accounts.licensed') }}</span>
          </p>
        </div>
        <Button
          v-tooltip="i18n('accounts.logout')"
          icon-only
          color="raised"
          @click="logout(selectedAccount.id)"
        >
          <TrashIcon />
        </Button>
      </div>

      <div v-else class="signed-out">
        <h4>{{ i18n('accounts.signedOut') }}</h4>
        <p>{{ i18n('accounts.choose') }}</p>
        <div class="auth-stack">
          <Button color="primary" class="auth-btn microsoft" @click="login()">
            <SSOMicrosoftIcon class="kind-svg" />
            {{ i18n('accounts.microsoft') }}
          </Button>
          <Button class="auth-btn" @click="tryElyLogin()">
            <GlobeIcon class="kind-svg" />
            {{ i18n('accounts.ely') }}
          </Button>
          <Button class="auth-btn" @click="tryOfflineLogin()">
            <PirateIcon class="kind-svg" />
            {{ i18n('accounts.offline') }}
          </Button>
        </div>
      </div>

      <div v-if="displayAccounts.length > 0" class="account-group">
        <div v-for="account in displayAccounts" :key="account.id" class="account-row">
          <Button class="option account" @click="setAccount(account)">
            <img
              class="face face-sm"
              :src="faceOf(account)"
              alt=""
              width="24"
              height="24"
              draggable="false"
            />
            <div class="account-copy">
              <p class="account-type">{{ account.username }}</p>
              <span>
                <span class="kind-badge inline" :data-kind="accountKind(account)">
                  <SSOMicrosoftIcon v-if="accountKind(account) === 'microsoft'" class="kind-svg" />
                  <GlobeIcon v-else-if="accountKind(account) === 'ely'" class="kind-svg" />
                  <PirateIcon v-else class="kind-svg" />
                </span>
                {{ accountKindLabel(account) }}
              </span>
            </div>
          </Button>
          <Button v-tooltip="i18n('accounts.logout')" icon-only @click="logout(account.id)">
            <TrashIcon />
          </Button>
        </div>
      </div>

      <div v-if="accounts.length > 0" class="add-more">
        <Button class="auth-btn" @click="login()">
          <SSOMicrosoftIcon class="kind-svg" />
          {{ i18n('accounts.microsoft') }}
        </Button>
        <Button class="auth-btn" @click="tryElyLogin()">
          <GlobeIcon class="kind-svg" />
          {{ i18n('accounts.ely') }}
        </Button>
        <Button class="auth-btn" @click="tryOfflineLogin()">
          <PirateIcon class="kind-svg" />
          {{ i18n('accounts.offline') }}
        </Button>
      </div>
    </Card>
  </transition>

  <ModalWrapper ref="loginElyModal" class="modal" header="Ely.by">
    <div class="modal-body stacked">
      <div class="label">Ник или email и пароль от ely.by — скины и вход на сервера с Ely.</div>
      <input v-model="elyUsername" type="text" maxlength="64" placeholder="username or email" />
      <input v-model="elyPassword" type="password" placeholder="password" />
      <Button color="primary" @click="elyLoginFinally()">Войти через Ely.by</Button>
    </div>
  </ModalWrapper>

  <ModalWrapper ref="loginOfflineModal" class="modal" header="Offline аккаунт">
    <div class="modal-body stacked">
      <div class="label">Ник, 3–16 символов. Без Microsoft, сразу можно играть.</div>
      <input v-model="playerName" type="text" maxlength="16" placeholder="Steve" />
      <Button color="primary" @click="offlineLoginFinally()">Войти offline</Button>
    </div>
  </ModalWrapper>

  <ModalWrapper ref="loginErrorModal" class="modal" header="Ошибка">
    <div class="modal-body stacked">
      <div class="label">Не удалось добавить аккаунт. Проверь данные и попробуй снова.</div>
      <Button color="primary" @click="retryOfflineLogin()">Повторить</Button>
    </div>
  </ModalWrapper>

  <ModalWrapper ref="unexpectedErrorModal" class="modal" header="Ошибка">
    <div class="modal-body">
      <div class="label">Неожиданная ошибка</div>
    </div>
  </ModalWrapper>

  <ModalWrapper ref="msDeviceModal" class="modal" :header="i18n('accounts.msDeviceTitle')">
    <div class="modal-body stacked">
      <div class="label">{{ msDeviceMessage || i18n('accounts.msDeviceHint') }}</div>
      <div class="ms-code">{{ msDeviceCode }}</div>
      <p class="ms-link">{{ msDeviceUri }}</p>
      <p class="ms-wait">{{ i18n('accounts.msDeviceWait') }}</p>
      <div class="ms-actions">
        <Button color="primary" @click="copyMsCode">{{ i18n('accounts.msDeviceCopy') }}</Button>
        <Button @click="reopenMsBrowser">{{ i18n('accounts.msDeviceOpen') }}</Button>
      </div>
    </div>
  </ModalWrapper>
  </div>
</template>

<script setup>
import { TrashIcon, PirateIcon, GlobeIcon, SSOMicrosoftIcon } from '@modrinth/assets'
import { Button, Card } from '@modrinth/ui'
import { ref, computed, onMounted, onBeforeUnmount, onUnmounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { openUrl } from '@/helpers/utils.js'
import {
  playerFaceSrc,
  playerFaceFallback,
  clearPlayerFaceCache,
  invalidatePlayerFace,
} from '@/helpers/playerFace.js'
import {
  offline_login,
  ely_login,
  users,
  remove_user,
  set_default_user,
  login as login_flow,
  get_default_user,
} from '@/helpers/auth'
import { handleError } from '@/store/state.js'
import { process_listener } from '@/helpers/events'
import { handleSevereError } from '@/store/error.js'
import ModalWrapper from './modal/ModalWrapper.vue'
import { t as translate, useI18n } from '@/helpers/i18n.js'

const { t: i18n } = useI18n()

defineProps({
  mode: {
    type: String,
    required: true,
    default: 'normal',
  },
})

const emit = defineEmits(['change'])

const accounts = ref([])
const defaultUser = ref()
const faces = ref({})
const loading = ref(false)
const unsignedFace = playerFaceFallback()

const loginOfflineModal = ref(null)
const loginElyModal = ref(null)
const loginErrorModal = ref(null)
const unexpectedErrorModal = ref(null)
const msDeviceModal = ref(null)
const msDeviceCode = ref('')
const msDeviceUri = ref('https://www.microsoft.com/link')
const msDeviceOpenUri = ref('https://www.microsoft.com/link')
const msDeviceMessage = ref('')
const playerName = ref('')
const elyUsername = ref('')
const elyPassword = ref('')
const showCard = ref(false)
const card = ref(null)
const button = ref(null)

let unlistenProcess = () => {}
let unlistenMs = () => {}

function accountKind(account) {
  if (!account) return 'offline'
  if (account.kind === 'ely') return 'ely'
  if (account.kind === 'microsoft' || account.sessionOk) return 'microsoft'
  return 'offline'
}

function accountKindLabel(account) {
  const kind = accountKind(account)
  if (kind === 'ely') return translate('accounts.ely')
  if (kind === 'microsoft') return translate('accounts.microsoft')
  return translate('accounts.offline')
}

function faceOf(account) {
  if (!account?.username) return unsignedFace
  return faces.value[account.username] || unsignedFace
}

const displayAccounts = computed(() =>
  accounts.value.filter((account) => defaultUser.value !== account.id),
)

const selectedAccount = computed(() =>
  accounts.value.find((account) => account.id === defaultUser.value),
)

const selectedFace = computed(() => faceOf(selectedAccount.value))

function copyMsCode() {
  const code = msDeviceCode.value
  if (!code) return
  navigator.clipboard?.writeText(code).catch(() => {})
}

function reopenMsBrowser() {
  const url = msDeviceOpenUri.value || msDeviceUri.value
  if (!url) return
  openUrl(url).catch(() => window.open(url, '_blank'))
}

function tryOfflineLogin() {
  loginOfflineModal.value?.show()
}

function retryOfflineLogin() {
  loginErrorModal.value?.hide()
  tryOfflineLogin()
}

function tryElyLogin() {
  loginElyModal.value?.show()
}

async function offlineLoginFinally() {
  const name = playerName.value.trim()
  if (name.length >= 3 && name.length <= 16) {
    const loggedIn = await offline_login(name).catch(handleError)
    loginOfflineModal.value?.hide()
    playerName.value = ''
    if (loggedIn) {
      await setAccount(loggedIn)
      await refreshValues()
    } else {
      unexpectedErrorModal.value?.show()
    }
  } else {
    playerName.value = ''
    loginOfflineModal.value?.hide()
    loginErrorModal.value?.show()
  }
}

async function elyLoginFinally() {
  const name = elyUsername.value.trim()
  const password = elyPassword.value
  if (!name || !password) {
    loginElyModal.value?.hide()
    loginErrorModal.value?.show()
    return
  }
  const loggedIn = await ely_login(name, password).catch(handleSevereError)
  elyPassword.value = ''
  loginElyModal.value?.hide()
  if (loggedIn) {
    await setAccount(loggedIn)
    await refreshValues()
    elyUsername.value = ''
  }
}

async function refreshFaces() {
  const list = accounts.value.slice()
  await Promise.all(
    list.map(async (account) => {
      const name = account?.username
      if (!name) return
      const src = await playerFaceSrc(name, account.uuid)
      faces.value = { ...faces.value, [name]: src }
    }),
  )
}

async function refreshValues() {
  if (loading.value) return
  loading.value = true
  try {
    defaultUser.value = await get_default_user().catch(handleError)
    const list = await users().catch(handleError)
    accounts.value = Array.isArray(list) ? list : []
    // Faces load in background — UI already painted with placeholders.
    refreshFaces().catch(() => {})
  } finally {
    loading.value = false
  }
}

defineExpose({ refreshValues })

async function setAccount(account) {
  defaultUser.value = account.id
  await set_default_user(account.id).catch(handleError)
  emit('change')
}

async function login() {
  const loggedIn = await login_flow().catch(handleSevereError)
  if (loggedIn) {
    clearPlayerFaceCache()
    invalidatePlayerFace(loggedIn.username, loggedIn.uuid)
    await setAccount(loggedIn)
    await refreshValues()
  }
}

async function logout(id) {
  await remove_user(id).catch(handleError)
  await refreshValues()
  if (!selectedAccount.value && accounts.value.length > 0) {
    await setAccount(accounts.value[0])
    await refreshValues()
  } else {
    emit('change')
  }
}

function toggleMenu(override = true) {
  if (showCard.value || !override) showCard.value = false
  else showCard.value = true
}

function handleClickOutside(event) {
  const elements = document.elementsFromPoint(event.clientX, event.clientY)
  if (
    card.value &&
    card.value.$el !== event.target &&
    !elements.includes(card.value.$el) &&
    button.value &&
    !button.value.contains(event.target)
  ) {
    toggleMenu(false)
  }
}

onMounted(() => {
  window.addEventListener('click', handleClickOutside)
  refreshValues().catch(() => {})
  process_listener(async (e) => {
    if (e.event === 'launched') await refreshValues()
  })
    .then((fn) => {
      unlistenProcess = fn
    })
    .catch(() => {})
  listen('msa-device-code', (event) => {
    const payload = event.payload || {}
    if (payload.done) {
      msDeviceModal.value?.hide?.()
      return
    }
    msDeviceCode.value = String(payload.userCode || '')
    msDeviceUri.value = String(payload.verificationUri || 'https://www.microsoft.com/link')
    msDeviceOpenUri.value = String(
      payload.openUri || payload.verificationUri || 'https://www.microsoft.com/link',
    )
    msDeviceMessage.value = String(payload.message || '')
    msDeviceModal.value?.show?.()
  })
    .then((fn) => {
      unlistenMs = fn
    })
    .catch(() => {})
})

onBeforeUnmount(() => {
  window.removeEventListener('click', handleClickOutside)
})

onUnmounted(() => {
  unlistenProcess()
  unlistenMs()
})
</script>

<style scoped lang="scss">
.accounts-shell {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 100%;
  flex-shrink: 0;
}

.face {
  width: 3rem;
  height: 3rem;
  min-width: 3rem;
  min-height: 3rem;
  max-width: 3rem;
  max-height: 3rem;
  border-radius: var(--radius-sm);
  object-fit: cover;
  image-rendering: pixelated;
  display: block;
  background: var(--color-button-bg);
  flex-shrink: 0;
}

.face-md {
  width: 2.5rem;
  height: 2.5rem;
  min-width: 2.5rem;
  min-height: 2.5rem;
  max-width: 2.5rem;
  max-height: 2.5rem;
}

.face-sm {
  width: 1.5rem;
  height: 1.5rem;
  min-width: 1.5rem;
  min-height: 1.5rem;
  max-width: 1.5rem;
  max-height: 1.5rem;
}

.kind-svg {
  width: 0.95rem !important;
  height: 0.95rem !important;
  min-width: 0.95rem !important;
  min-height: 0.95rem !important;
  display: block;
  flex-shrink: 0;
}

.kind-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.15rem;
  height: 1.15rem;
  border-radius: 0.25rem;
  background: rgba(255, 255, 255, 0.08);
  flex-shrink: 0;

  &.inline {
    margin-right: 0.25rem;
    vertical-align: middle;
  }

  &[data-kind='microsoft'] {
    background: rgba(0, 164, 239, 0.18);
  }
  &[data-kind='ely'] {
    background: rgba(80, 200, 120, 0.18);
  }
  &[data-kind='offline'] {
    background: rgba(255, 180, 60, 0.18);
  }
}

.session-ok {
  margin-left: 0.35rem;
  color: #6dce8a;
  font-weight: 600;
  font-size: 0.75rem;
}

.selected {
  background: var(--color-brand-highlight);
  border-radius: var(--radius-lg);
  color: var(--color-contrast);
  gap: 0.85rem;
  width: 100%;
  display: flex;
  align-items: center;
  padding: 0.55rem 0.65rem;
  box-sizing: border-box;
}

.selected-meta {
  min-width: 0;
  flex: 1;

  h4,
  p {
    margin: 0;
  }

  p {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    opacity: 0.9;
    font-size: 0.8rem;
    flex-wrap: wrap;
  }
}

.signed-out {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  width: 16.5rem;

  h4,
  p {
    margin: 0;
  }

  h4 {
    color: var(--color-contrast);
    font-size: 1.05rem;
  }
}

.auth-stack,
.add-more {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
  margin-top: 0.35rem;
}

.auth-btn {
  justify-content: flex-start;
  gap: 0.55rem;

  :deep(svg),
  .kind-svg {
    width: 1.1rem !important;
    height: 1.1rem !important;
    min-width: 1.1rem !important;
    min-height: 1.1rem !important;
  }
}

.microsoft {
  background: linear-gradient(90deg, #107c10 0%, #00a4ef 100%);
  color: #fff;
}

.account-group {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  margin-top: 0.35rem;
}

.account-row {
  display: flex;
  flex-direction: row;
  gap: 0.35rem;
  align-items: center;
}

.option {
  flex: 1;
  min-width: 0;
  justify-content: flex-start;
  gap: 0.55rem;
  background: var(--color-raised-bg);
  box-shadow: none;
}

.account-copy {
  display: flex;
  flex-direction: column;
  min-width: 0;
  text-align: left;

  .account-type {
    margin: 0;
    font-weight: 600;
    color: var(--color-contrast);
  }

  span {
    display: inline-flex;
    align-items: center;
    font-size: 0.75rem;
    color: var(--color-secondary);
  }
}

.account-card {
  position: absolute;
  display: flex;
  flex-direction: column;
  top: 0.5rem;
  left: 5.5rem;
  z-index: 11;
  gap: 0.65rem;
  padding: 0.9rem;
  border: 1px solid var(--color-button-bg);
  width: 19rem;
  max-height: 98vh;
  overflow-y: auto;
  user-select: none;

  &.isolated {
    position: relative;
    left: 0;
    top: 0;
    width: 100%;
  }

  &.expanded {
    width: 100%;
    left: 0;
  }
}

.avatar-button {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-base);
  background-color: var(--color-raised-bg);
  border-radius: var(--radius-md);
  width: 3rem;
  height: 3rem;
  min-width: 3rem;
  min-height: 3rem;
  padding: 0;
  overflow: hidden;
  margin-inline: auto;
  flex-shrink: 0;

  &.expanded {
    width: 100%;
    height: auto;
    justify-content: flex-start;
    padding: 0.75rem;
  }
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.modal-body {
  display: flex;
  flex-direction: row;
  gap: var(--gap-lg);
  align-items: center;
  padding: var(--gap-xl);

  &.stacked {
    flex-direction: column;
    align-items: stretch;

    input {
      width: 100%;
    }
  }
}

.ms-code {
  font-family: var(--mono-font, ui-monospace, monospace);
  font-size: 1.85rem;
  font-weight: 700;
  letter-spacing: 0.2em;
  text-align: center;
  color: var(--color-contrast);
  background: var(--color-bg);
  border: 1px solid var(--color-button-bg);
  border-radius: var(--radius-md);
  padding: 0.75rem 1rem;
  user-select: all;
}

.ms-link {
  margin: 0;
  font-size: 0.85rem;
  color: var(--color-secondary);
  word-break: break-all;
}

.ms-wait {
  margin: 0;
  font-size: 0.9rem;
  color: var(--color-brand);
}

.ms-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.label {
  color: var(--color-base);
  font-size: 0.9rem;
  line-height: 1.4;
}
</style>
