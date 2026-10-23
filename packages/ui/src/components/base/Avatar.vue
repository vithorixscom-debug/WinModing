<template>
  <img
    v-if="shownSrc"
    ref="img"
    :style="`--_size: ${cssSize}`"
    class="experimental-styles-within avatar"
    :class="{
      circle: circle,
      'no-shadow': noShadow,
      raised: raised,
      pixelated: pixelated,
    }"
    :src="shownSrc"
    :alt="alt"
    :loading="loading"
    @load="updatePixelated"
    @error="failed = true"
  />
  <svg
    v-else
    class="experimental-styles-within avatar"
    :style="`--_size: ${cssSize}${tint != null ? `;--_tint:oklch(50% 75% ${tint})` : ''}`"
    :class="{
      tint: tint != null,
      circle: circle,
      'no-shadow': noShadow,
      raised: raised,
    }"
    xml:space="preserve"
    fill-rule="evenodd"
    stroke-linecap="round"
    stroke-linejoin="round"
    stroke-miterlimit="1.5"
    clip-rule="evenodd"
    viewBox="0 0 104 104"
    aria-hidden="true"
  >
    <path fill="none" d="M0 0h103.4v103.4H0z" />
    <path
      fill="none"
      stroke="#9a9a9a"
      stroke-width="5"
      d="M51.7 92.5V51.7L16.4 31.3l35.3 20.4L87 31.3 51.7 11 16.4 31.3v40.8l35.3 20.4L87 72V31.3L51.7 11"
    />
  </svg>
</template>

<script setup>
import { ref, computed, watch } from 'vue'

const props = defineProps({
  src: {
    type: String,
    default: null,
  },
  alt: {
    type: String,
    default: '',
  },
  size: {
    type: String,
    default: '2rem',
  },
  circle: {
    type: Boolean,
    default: false,
  },
  noShadow: {
    type: Boolean,
    default: false,
  },
  loading: {
    type: String,
    default: 'eager',
  },
  raised: {
    type: Boolean,
    default: false,
  },
  tintBy: {
    type: String,
    default: null,
  },
})

const pixelated = ref(false)
const failed = ref(false)
const img = ref(null)
const shownSrc = computed(() => (failed.value ? null : props.src))
watch(
  () => props.src,
  () => {
    failed.value = false
  },
)

const tint = computed(() => {
  if (!props.tintBy) return null
  let hash = 0
  for (let i = 0; i < props.tintBy.length; i++) {
    hash = (hash << 5) - hash + props.tintBy.charCodeAt(i)
    hash |= 0
  }
  return Math.abs(hash) % 360
})

const LEGACY_PRESETS = {
  xxs: '1.25rem',
  xs: '2.5rem',
  sm: '3rem',
  md: '6rem',
  lg: '9rem',
}

const cssSize = computed(() => LEGACY_PRESETS[props.size] ?? props.size)

function updatePixelated() {
  pixelated.value = !!(
    props.raised &&
    img.value &&
    img.value.naturalWidth &&
    img.value.naturalWidth <= 32
  )
}

</script>

<style lang="scss" scoped>
.avatar {
  --_size: 2rem;
  width: var(--_size) !important;
  height: var(--_size) !important;
  min-width: var(--_size) !important;
  min-height: var(--_size) !important;
  max-width: var(--_size) !important;
  max-height: var(--_size) !important;

  border: 1px solid var(--color-button-border);
  background-color: var(--color-button-bg);
  object-fit: cover;
  overflow: hidden;
  flex-shrink: 0;
  border-radius: calc(16 / 96 * var(--_size));

  &.circle {
    border-radius: 50%;
  }

  &:not(.no-shadow) {
    box-shadow: var(--shadow-inset-lg), var(--shadow-card);
  }

  &.no-shadow {
    box-shadow: none;
  }

  &.pixelated {
    image-rendering: pixelated;
  }

  &.raised {
    background-color: var(--color-raised-bg);
  }

  &.tint {
    background-color: color-mix(in oklch, var(--color-button-bg) 100%, var(--_tint) 8%);
  }
}
</style>
