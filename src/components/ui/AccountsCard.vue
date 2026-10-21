<template>
  <div v-if="mode !== 'isolated'" ref="button" v-tooltip.right="i18n('accounts.tooltip')" class="button-base avatar-button"
    :class="{ expanded: mode === 'expanded' }" @click="toggleMenu">
    <Avatar :size="mode === 'expanded' ? 'xs' : '3rem'" :no-shadow="true" :src="selectedAccount
      ? `https://mc-heads.net/avatar/${selectedAccount.username}/128`
      : 'https://launcher-files.modrinth.com/assets/steve_head.png'
      " />
  </div>
  <transition name="fade">
    <Card v-if="showCard || mode === 'isolated'" ref="card" class="account-card"
      :class="{ expanded: mode === 'expanded', isolated: mode === 'isolated' }">
      <div v-if="selectedAccount" class="selected account">
        <Avatar size="xs" :src="`https://mc-heads.net/avatar/${selectedAccount.username}/128`" />
        <div class="selected-meta">
          <h4>{{ selectedAccount.username }}</h4>
          <p>
            <component :is="getAccountType(selectedAccount)" class="vector-icon" />
            {{ accountKindLabel(selectedAccount) }}
          </p>
        </div>
        <Button v-tooltip="i18n('accounts.logout')" icon-only color="raised" @click="logout(selectedAccount.id)">
          <TrashIcon />
        </Button>
      </div>
      <div v-else class="signed-out">
        <h4>{{ i18n('accounts.signedOut') }}</h4>
        <p>{{ i18n('accounts.choose') }}</p>
        <div class="auth-stack">
          <Button color="primary" class="auth-btn microsoft" @click="login()">
            <MicrosoftIcon />
            {{ i18n('accounts.microsoft') }}
          </Button>
          <Button class="auth-btn" @click="tryElyLogin()">
            <GlobeIcon />
            {{ i18n('accounts.ely') }}
          </Button>
          <Button class="auth-btn" @click="tryOfflineLogin()">
            <PirateIcon />
            {{ i18n('accounts.offline') }}
          </Button>
        </div>
      </div>
      <div v-if="displayAccounts.length > 0" class="account-group">
        <div v-for="account in displayAccounts" :key="account.id" class="account-row">
          <Button class="option account" @click="setAccount(account)">
            <Avatar :src="`https://mc-heads.net/avatar/${account.username}/128`" class="icon" />
            <div class="account-copy">
              <p class="account-type">{{ account.username }}</p>
              <span>{{ accountKindLabel(account) }}</span>
            </div>
          </Button>
          <Button v-tooltip="i18n('accounts.logout')" icon-only @click="logout(account.id)">
            <TrashIcon />
          </Button>
        </div>
      </div>
      <div v-if="accounts.length > 0" class="add-more">
        <Button class="auth-btn" @click="login()">
          <MicrosoftIcon />
          {{ i18n('accounts.microsoft') }}
        </Button>
        <Button class="auth-btn" @click="tryElyLogin()">
          <GlobeIcon />
          {{ i18n('accounts.ely') }}
        </Button>
        <Button class="auth-btn" @click="tryOfflineLogin()">
          <PirateIcon />
          {{ i18n('accounts.offline') }}
        </Button>
      </div>
    </Card>
  </transition>
  <ModalWrapper ref="loginElyModal" class="modal" header="Ely.by">
    <div class="modal-body stacked">
      <div class="label">Ник или email и пароль от ely.by — скины и вход на сервера с Ely.</div>
      <input type="text" v-model="elyUsername" maxlength="64" placeholder="username or email" />
      <input type="password" v-model="elyPassword" placeholder="password" />
      <Button color="primary" @click="elyLoginFinally()">
        Войти через Ely.by
      </Button>
    </div>
  </ModalWrapper>
  <ModalWrapper ref="loginOfflineModal" class="modal" header="Offline аккаунт">
    <div class="modal-body">
      <div class="label">Ник, 3–16 символов. Без Microsoft, сразу можно играть.</div>
      <input type="text" v-model="playerName" maxlength="16" placeholder="Steve" />
      <Button color="primary" @click="offlineLoginFinally()">
        Войти offline
      </Button>
    </div>
  </ModalWrapper>
  <ModalWrapper ref="loginErrorModal" class="modal" header="Error while proceed">
    <div class="modal-body">
      <div class="label">Error occurred while adding offline account</div>
      <Button color="primary" @click="retryOfflineLogin()">
        Try again
      </Button>
    </div>
  </ModalWrapper>
  <ModalWrapper ref="unexpectedErrorModal" class="modal" header="Ошибка">
    <div class="modal-body">
      <div class="label">Unexcepted error</div>
    </div>
  </ModalWrapper>
</template>

<script setup>
import { PlusIcon, TrashIcon, LogInIcon, PirateIcon as Offline, MicrosoftIcon as License, MicrosoftIcon, PirateIcon, GlobeIcon } from '@modrinth/assets'
import { Avatar, Button, Card } from '@modrinth/ui'
import { ref, computed, onMounted, onBeforeUnmount, onUnmounted } from 'vue'
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
const loginOfflineModal = ref(null)
const loginElyModal = ref(null)
const loginErrorModal = ref(null)
const unexpectedErrorModal = ref(null)
const playerName = ref('')
const elyUsername = ref('')
const elyPassword = ref('')

async function tryOfflineLogin() { // Patched
  loginOfflineModal.value.show()
}

async function offlineLoginFinally() { // Patched
  let name = playerName.value
  if (name.length > 1 && name.length < 20 && name !== '') {
    const loggedIn = await offline_login(name).catch(handleError)
    loginOfflineModal.value.hide()
    if (loggedIn) {
      await setAccount(loggedIn)
      await refreshValues()
    } else {
      unexpectedErrorModal.value.show()
    }
    playerName.value = ''
  } else {
    playerName.value = ''
    loginOfflineModal.value.hide()
    loginErrorModal.value.show()
  }
}

function retryOfflineLogin() { // Patched
  loginErrorModal.value.hide()
  tryOfflineLogin()
}

function tryElyLogin() {
  loginElyModal.value.show()
}

async function elyLoginFinally() {
  const name = elyUsername.value.trim()
  const password = elyPassword.value
  if (!name || !password) {
    loginElyModal.value.hide()
    loginErrorModal.value.show()
    return
  }
  const loggedIn = await ely_login(name, password).catch(handleSevereError)
  elyPassword.value = ''
  loginElyModal.value.hide()
  if (loggedIn) {
    await setAccount(loggedIn)
    await refreshValues()
    elyUsername.value = ''
  }
}

function getAccountType(account) {
  if (account.kind === 'ely') return GlobeIcon
  if (account.kind === 'microsoft' || (account.access_token && account.access_token !== 'null')) {
    return License
  }
  return Offline
}

function accountKindLabel(account) {
  if (account.kind === 'ely') return translate('accounts.ely')
  if (account.kind === 'microsoft' || (account.access_token && account.access_token !== 'null')) {
    return translate('accounts.microsoft')
  }
  return translate('accounts.offline')
}

async function refreshValues() {
  defaultUser.value = await get_default_user().catch(handleError)
  const list = await users().catch(handleError)
  accounts.value = Array.isArray(list) ? list : []
}
defineExpose({
  refreshValues,
})
await refreshValues()

const displayAccounts = computed(() =>
  accounts.value.filter((account) => defaultUser.value !== account.id),
)

const selectedAccount = computed(() =>
  accounts.value.find((account) => account.id === defaultUser.value),
)

async function setAccount(account) {
  defaultUser.value = account.id
  await set_default_user(account.id).catch(handleError)
  emit('change')
}

async function login() {
  const loggedIn = await login_flow().catch(handleSevereError)

  if (loggedIn) {
    await setAccount(loggedIn)
    await refreshValues()
  }
}

const logout = async (id) => {
  await remove_user(id).catch(handleError)
  await refreshValues()
  if (!selectedAccount.value && accounts.value.length > 0) {
    await setAccount(accounts.value[0])
    await refreshValues()
  } else {
    emit('change')
  }
}

const showCard = ref(false)
const card = ref(null)
const button = ref(null)
const handleClickOutside = (event) => {
  const elements = document.elementsFromPoint(event.clientX, event.clientY)
  if (
    card.value &&
    card.value.$el !== event.target &&
    !elements.includes(card.value.$el) &&
    !button.value.contains(event.target)
  ) {
    toggleMenu(false)
  }
}

function toggleMenu(override = true) {
  if (showCard.value || !override) {
    showCard.value = false
  } else {
    showCard.value = true
  }
}

const unlisten = await process_listener(async (e) => {
  if (e.event === 'launched') {
    await refreshValues()
  }
})

onMounted(() => {
  window.addEventListener('click', handleClickOutside)
})

onBeforeUnmount(() => {
  window.removeEventListener('click', handleClickOutside)
})

onUnmounted(() => {
  unlisten()
})
</script>

<style scoped lang="scss">
.selected {
  background: var(--color-brand-highlight);
  border-radius: var(--radius-lg);
  color: var(--color-contrast);
  gap: 0.85rem;
  width: 100%;
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
    opacity: 0.8;
    font-size: 0.8rem;
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

  p {
    color: var(--color-secondary);
    margin-bottom: 0.4rem;
  }
}

.auth-stack,
.add-more {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  width: 100%;
}

.auth-btn {
  width: 100%;
  justify-content: flex-start;
  gap: 0.55rem;
}

.microsoft {
  font-weight: 700;
}


.vector-icon {
  width: 12px;
  height: 12px;
}

.account {
  width: max-content;
  display: flex;
  align-items: center;
  text-align: left;
  padding: 0.5rem 1rem;

  h4,
  p {
    margin: 0;
  }
}

.account-copy {
  display: flex;
  flex-direction: column;
  min-width: 0;

  span {
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
  width: 18.5rem;
  user-select: none;
  -ms-user-select: none;
  -webkit-user-select: none;
  max-height: 98vh;
  overflow-y: auto;

  &::-webkit-scrollbar-track {
    border-top-right-radius: 1rem;
    border-bottom-right-radius: 1rem;
  }

  &::-webkit-scrollbar {
    border-top-right-radius: 1rem;
    border-bottom-right-radius: 1rem;
  }

  &.hidden {
    display: none;
  }

  &.expanded {
    left: 13.5rem;
  }

  &.isolated {
    position: relative;
    left: 0;
    top: 0;
  }
}

.accounts-title {
  font-size: 1.2rem;
  font-weight: bolder;
}

.centered {
  display: flex;
  gap: 1rem;
  margin: auto;
}

.account-group {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.option {
  width: calc(100% - 2.25rem);
  background: var(--color-raised-bg);
  color: var(--color-base);
  box-shadow: none;

  img {
    margin-right: 0.5rem;
  }
}

.icon {
  --size: 1.5rem !important;
}

.account-row {
  display: flex;
  flex-direction: row;
  gap: 0.5rem;
  vertical-align: center;
  justify-content: space-between;
  padding-right: 1rem;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.avatar-button {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  color: var(--color-base);
  background-color: var(--color-raised-bg);
  border-radius: var(--radius-md);
  width: 3rem;
  height: 3rem;
  padding: 0;
  overflow: hidden;
  text-align: center;
  margin-inline: auto;

  &.expanded {
    width: 100%;
    height: auto;
    justify-content: flex-start;
    text-align: left;
    border: 1px solid var(--color-button-bg);
    padding: 1rem;
  }
}

.avatar-text {
  margin: auto 0 auto 0.25rem;
  display: flex;
  flex-direction: column;
}

.text {
  width: 6rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.accounts-text {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  margin: 0;
}

.qr-code {
  background-color: white !important;
  border-radius: var(--radius-md);
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

  .modal-text {
    display: flex;
    flex-direction: column;
    gap: var(--gap-sm);
    width: 100%;

    h2,
    p {
      margin: 0;
    }

    .code-text {
      display: flex;
      flex-direction: row;
      gap: var(--gap-xs);
      align-items: center;

      .code {
        background-color: var(--color-bg);
        border-radius: var(--radius-md);
        border: solid 1px var(--color-button-bg);
        font-family: var(--mono-font);
        letter-spacing: var(--gap-md);
        color: var(--color-contrast);
        font-size: 2rem;
        font-weight: bold;
        padding: var(--gap-sm) 0 var(--gap-sm) var(--gap-md);
      }

      .btn {
        width: 2.5rem;
        height: 2.5rem;
      }
    }
  }
}

.button-row {
  display: flex;
  flex-direction: row;
}

.modal {
  position: absolute;
}

.code {
  color: var(--color-brand);
  padding: 0.05rem 0.1rem;
  // row not column
  display: flex;

  .card {
    background: var(--color-base);
    color: var(--color-contrast);
    padding: 0.5rem 1rem;
  }
}
</style>
