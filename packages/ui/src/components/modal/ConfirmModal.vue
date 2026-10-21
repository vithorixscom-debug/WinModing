<template>
  <Modal ref="modal" :header="title" :noblur="noblur" :on-hide="onHide">
    <div class="confirm-body">
      <p class="confirm-copy">{{ description }}</p>
      <label v-if="hasToType" class="confirmation-label" for="confirmation">
        Type <em>{{ confirmationText }}</em> to confirm
      </label>
      <input
        v-if="hasToType"
        id="confirmation"
        v-model="confirmation_typed"
        class="confirm-input"
        type="text"
        @input="type"
      />
      <div class="confirm-actions">
        <button class="btn confirm-delete" type="button" :disabled="action_disabled" @click="proceed">
          <TrashIcon />
          {{ proceedLabel }}
        </button>
        <button class="btn" type="button" @click="modal.hide()">
          <XIcon />
          {{ cancelLabel }}
        </button>
      </div>
    </div>
  </Modal>
</template>

<script setup>
import { ref } from 'vue'
import { TrashIcon, XIcon } from '@modrinth/assets'
import Modal from './Modal.vue'

const props = defineProps({
  confirmationText: { type: String, default: '' },
  hasToType: { type: Boolean, default: false },
  title: { type: String, default: 'Confirm', required: true },
  description: { type: String, default: '', required: true },
  proceedLabel: { type: String, default: 'Proceed' },
  cancelLabel: { type: String, default: 'Cancel' },
  noblur: { type: Boolean, default: false },
  onHide: {
    type: Function,
    default() {
      return () => {}
    },
  },
})

const emit = defineEmits(['proceed'])
const modal = ref(null)
const action_disabled = ref(props.hasToType)
const confirmation_typed = ref('')

function proceed() {
  modal.value.hide()
  emit('proceed')
}

function type() {
  if (props.hasToType) {
    action_disabled.value =
      confirmation_typed.value.toLowerCase() !== props.confirmationText.toLowerCase()
  }
}

function show() {
  confirmation_typed.value = ''
  action_disabled.value = props.hasToType
  modal.value.show()
}

defineExpose({ show, hide: () => modal.value.hide() })
</script>

<style scoped lang="scss">
.confirm-body {
  padding: 1.25rem 1.4rem 1.35rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  min-width: min(28rem, calc(100vw - 4rem));
}

.confirm-copy {
  margin: 0;
  color: var(--color-base);
  line-height: 1.45;
}

.confirmation-label {
  font-weight: 600;
}

.confirm-input {
  width: 100%;
}

.confirm-actions {
  display: flex;
  gap: 0.6rem;
  justify-content: flex-end;
  flex-wrap: wrap;
}

.confirm-delete {
  background: var(--color-red) !important;
  color: #fff !important;
}
</style>
