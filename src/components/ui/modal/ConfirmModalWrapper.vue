<script setup lang="ts">
import { ref } from 'vue'
import { ConfirmModal } from '@modrinth/ui'
import { useTheming } from '@/store/theme.js'
import { t as i18n } from '@/helpers/i18n.js'

const themeStore = useTheming()

defineProps({
  confirmationText: { type: String, default: '' },
  hasToType: { type: Boolean, default: false },
  title: { type: String, default: 'No title defined', required: true },
  description: { type: String, default: 'No description defined', required: true },
  proceedLabel: { type: String, default: 'Proceed' },
})

const emit = defineEmits(['proceed'])
const modal = ref(null)

defineExpose({
  show: () => modal.value.show(),
  hide: () => modal.value.hide(),
})
</script>

<template>
  <ConfirmModal
    ref="modal"
    :confirmation-text="confirmationText"
    :has-to-type="hasToType"
    :title="title"
    :description="description"
    :proceed-label="proceedLabel"
    :cancel-label="i18n('create.cancel')"
    :noblur="!themeStore.advancedRendering"
    @proceed="emit('proceed')"
  />
</template>
