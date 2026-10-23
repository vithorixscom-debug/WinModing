<template>
  <div v-if="shown">
    <div
      :class="{ shown: actuallyShown }"
      class="tauri-overlay"
      @click="() => (closable ? hide() : {})"
    />
    <div
      :class="{
        shown: actuallyShown,
        noblur: props.noblur,
      }"
      class="modal-overlay"
      @click="() => (closable ? hide() : {})"
    />
    <div class="modal-container" :class="{ shown: actuallyShown }">
      <div class="modal-body">
        <div v-if="props.header" class="header">
          <h1>{{ props.header }}</h1>
          <button v-if="closable" class="btn icon-only transparent" @click="hide">
            <XIcon />
          </button>
        </div>
        <div class="content">
          <slot />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { XIcon } from '@modrinth/assets'
import { ref } from 'vue'

const props = defineProps({
  header: {
    type: String,
    default: null,
  },
  noblur: {
    type: Boolean,
    default: false,
  },
  closable: {
    type: Boolean,
    default: true,
  },
  onHide: {
    type: Function,
    default() {
      return () => {}
    },
  },
})

const shown = ref(false)
const actuallyShown = ref(false)

function show() {
  shown.value = true
  setTimeout(() => {
    actuallyShown.value = true
  }, 50)
}

function hide() {
  props.onHide()
  actuallyShown.value = false
  setTimeout(() => {
    shown.value = false
  }, 300)
}

defineExpose({
  show,
  hide,
})
</script>

<style lang="scss" scoped>
.tauri-overlay {
  position: fixed;
  visibility: hidden;
  top: 0;
  left: 0;
  width: 100%;
  height: 100px;
  z-index: 20;

  &.shown {
    opacity: 1;
    visibility: visible;
  }
}

.modal-overlay {
  visibility: hidden;
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 19;
  transition: all 0.3s ease-in-out;

  @media (prefers-reduced-motion) {
    transition: none !important;
  }

  &.shown {
    opacity: 1;
    visibility: visible;
    background: hsla(0, 0%, 0%, 0.5);
    backdrop-filter: blur(3px);
  }

  &.noblur {
    backdrop-filter: none;
  }
}

.modal-container {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 21;
  visibility: hidden;
  pointer-events: none;

  &.shown {
    visibility: visible;
    .modal-body {
      opacity: 1;
      visibility: visible;
      transform: translateY(0);
    }
  }

  .modal-body {
    position: fixed;
    box-shadow: var(--shadow-raised), var(--shadow-inset);
    border-radius: 1rem;
    overflow: hidden;
    max-height: calc(100% - 2 * var(--gap-lg));
    width: min(34rem, calc(100% - 2 * var(--gap-lg)));
    pointer-events: auto;

    .header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      background-color: var(--color-bg);
      padding: var(--gap-md) var(--gap-lg);
      border-radius: 1rem 1rem 0 0;

      h1 {
        font-weight: bold;
        font-size: 1.25rem;
      }
    }

    .content {
      background-color: var(--color-raised-bg);
      min-width: 0;
    }

    transform: translateY(50vh);
    visibility: hidden;
    opacity: 0;
    transition: all 0.25s ease-in-out;

    @media (prefers-reduced-motion) {
      transition: none !important;
    }

    @media screen and (max-width: 650px) {
      width: calc(100% - 2 * var(--gap-lg));
    }
  }
}
</style>
