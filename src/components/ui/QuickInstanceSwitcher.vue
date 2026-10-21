<script setup>
import { list } from '@/helpers/profile'
import { handleError } from '@/store/notifications'
import dayjs from 'dayjs'
import { onUnmounted, ref } from 'vue'
import { profile_listener } from '@/helpers/events.js'
import { BoxIcon } from '@modrinth/assets'
import { Avatar } from '@modrinth/ui'
import { instanceIconSrc } from '@/helpers/instanceIcon.js'

const recentInstances = ref([])
const getInstances = async () => {
  const profiles = (await list().catch(handleError)) ?? []
  recentInstances.value = profiles
    .sort((a, b) => {
      const dateA = dayjs(a.last_played || a.date_created || 0)
      const dateB = dayjs(b.last_played || b.date_created || 0)
      if (dateA.isSame(dateB)) return a.name.localeCompare(b.name)
      return dateB - dateA
    })
    .slice(0, 6)
}

await getInstances()

const unlistenProfile = await profile_listener(async (event) => {
  if (event.event !== 'synced') {
    await getInstances()
  }
})

onUnmounted(() => {
  unlistenProfile?.()
})
</script>

<template>
  <RouterLink
    v-for="instance in recentInstances"
    :key="instance.path"
    v-tooltip.right="instance.name"
    :to="`/instance/${encodeURIComponent(instance.path)}`"
    class="btn icon-only collapsed-button instance-nav"
  >
    <Avatar
      :key="instance.iconDataUrl || instance.icon_path || instance.path"
      :src="instanceIconSrc(instance)"
      :tint-by="instance.path"
      size="1.5rem"
      :no-shadow="true"
      class="nav-avatar"
    />
  </RouterLink>
</template>

<style scoped lang="scss">
.instance-nav {
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
  overflow: hidden;
  box-sizing: border-box;
  margin-inline: auto;

  svg {
    width: 1.5rem !important;
    height: 1.5rem !important;
    max-width: 1.5rem !important;
    max-height: 1.5rem !important;
  }
}

.nav-avatar {
  width: 1.5rem !important;
  height: 1.5rem !important;
  min-width: 1.5rem;
  min-height: 1.5rem;
  border: none !important;
  background: transparent !important;
  box-shadow: none !important;
}
</style>
