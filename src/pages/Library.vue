<script setup>
import { onUnmounted, ref, shallowRef } from 'vue'
import GridDisplay from '@/components/GridDisplay.vue'
import { list } from '@/helpers/profile.js'
import { useRoute } from 'vue-router'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { profile_listener } from '@/helpers/events.js'
import { handleError } from '@/store/notifications.js'
import { Button } from '@modrinth/ui'
import { PlusIcon } from '@modrinth/assets'
import InstanceCreationModal from '@/components/ui/InstanceCreationModal.vue'
import { NewInstanceImage } from '@/assets/icons'
import { t as translate, useI18n } from '@/helpers/i18n.js'

const { t: i18n } = useI18n()

const route = useRoute()
const breadcrumbs = useBreadcrumbs()
const installationModal = ref(null)

breadcrumbs.setRootContext({ name: translate('nav.library'), link: route.path })

const instances = shallowRef(await list().catch(handleError))

const offline = ref(!navigator.onLine)
window.addEventListener('offline', () => {
  offline.value = true
})
window.addEventListener('online', () => {
  offline.value = false
})

async function refreshInstances() {
  instances.value = (await list().catch(handleError)) ?? []
}

const unlistenProfile = await profile_listener(async () => {
  await refreshInstances()
})
onUnmounted(() => {
  unlistenProfile()
})
</script>

<template>
  <div class="library-page">
    <header class="library-head">
      <h1>{{ i18n('nav.library') }}</h1>
      <Button color="primary" :disabled="offline" @click="installationModal?.show()">
        <PlusIcon />
        {{ i18n('library.create') }}
      </Button>
    </header>
    <GridDisplay
      v-if="instances.length > 0"
      :label="i18n('library.instances')"
      :instances="instances"
      compact
      @reordered="refreshInstances"
    />
    <div v-else class="no-instance">
      <div class="icon">
        <NewInstanceImage />
      </div>
      <h3>{{ i18n('library.empty') }}</h3>
      <Button color="primary" :disabled="offline" @click="installationModal?.show()">
        <PlusIcon />
        {{ i18n('library.create') }}
      </Button>
    </div>
    <InstanceCreationModal ref="installationModal" />
  </div>
</template>

<style lang="scss" scoped>
.library-page {
  display: flex;
  flex-direction: column;
  min-height: 100%;
}

.library-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1.25rem 1.25rem 0;

  h1 {
    margin: 0;
    font-size: 1.45rem;
  }
}

.no-instance {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: var(--gap-md);

  p,
  h3 {
    margin: 0;
  }

  .icon {
    svg {
      width: 10rem;
      height: 10rem;
    }
  }
}
</style>
