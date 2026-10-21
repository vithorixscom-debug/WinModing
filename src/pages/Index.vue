<script setup>
import { computed, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Button } from '@modrinth/ui'
import { PlusIcon } from '@modrinth/assets'
import RowDisplay from '@/components/RowDisplay.vue'
import { list } from '@/helpers/profile.js'
import { profile_listener } from '@/helpers/events'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { handleError } from '@/store/notifications.js'
import dayjs from 'dayjs'
import { t as translate, useI18n } from '@/helpers/i18n.js'

const { t: i18n } = useI18n()

const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
breadcrumbs.setRootContext({ name: translate('nav.home'), link: route.path })

const profiles = ref([])

const getInstances = async () => {
  const listProfiles = (await list().catch(handleError)) ?? []
  profiles.value = listProfiles.sort((a, b) => {
    const dateA = dayjs(a.last_played ?? 0)
    const dateB = dayjs(b.last_played ?? 0)
    if (dateA.isSame(dateB)) return a.name.localeCompare(b.name)
    return dateB - dateA
  })
}

const quickRows = computed(() => [
  {
    label: 'Quick play',
    route: '/library',
    instances: profiles.value,
    instance: true,
    downloaded: true,
    compact: true,
  },
])

await getInstances()

const unlistenProfile = await profile_listener(async () => {
  await getInstances()
})

onUnmounted(() => {
  unlistenProfile?.()
})
</script>

<template>
  <div class="p-6 flex flex-col gap-2">
    <h1 v-if="profiles.length" class="m-0 text-2xl">{{ i18n('home.welcome') }}</h1>
    <h1 v-else class="m-0 text-2xl">{{ i18n('home.welcomeEmpty') }}</h1>

    <RowDisplay v-if="profiles.length > 0" :instances="quickRows" :can-paginate="true" />
    <div v-else class="empty-home">
      <p>{{ i18n('home.empty') }}</p>
      <Button color="primary" @click="router.push('/library')">
        <PlusIcon />
        {{ i18n('home.create') }}
      </Button>
    </div>
  </div>
</template>

<style scoped lang="scss">
.empty-home {
  display: flex;
  flex-direction: column;
  gap: var(--gap-sm);
  color: var(--color-secondary);
  padding: 1rem 0;
}
</style>
