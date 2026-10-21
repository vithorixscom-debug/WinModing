<template>
  <div ref="root" class="chips">
    <div class="chip-indicator" :style="indicatorStyle" aria-hidden="true" />
    <Button
      v-for="(item, index) in items"
      :key="typeof item === 'object' ? item?.name ?? index : item"
      class="btn"
      :class="{ selected: selected === item, capitalize: capitalize }"
      @click="toggleItem(item)"
    >
      <CheckIcon v-if="selected === item" />
      <span>{{ formatLabel(item) }}</span>
    </Button>
  </div>
</template>
<script setup>
import { CheckIcon } from '@modrinth/assets'
</script>
<script>
import { defineComponent } from 'vue'
import Button from './Button.vue'

export default defineComponent({
  props: {
    modelValue: {
      required: true,
      type: String,
    },
    items: {
      required: true,
      type: Array,
    },
    neverEmpty: {
      default: true,
      type: Boolean,
    },
    formatLabel: {
      default: (x) => x,
      type: Function,
    },
    capitalize: {
      type: Boolean,
      default: true,
    },
  },
  emits: ['update:modelValue'],
  data() {
    return {
            indicator: { x: 0, y: 0, w: 0, h: 36, ready: false },
    }
  },
  computed: {
    selected: {
      get() {
        return this.modelValue
      },
      set(value) {
        this.$emit('update:modelValue', value)
      },
    },
    indicatorStyle() {
        return {
        transform: `translate(${this.indicator.x}px, ${this.indicator.y}px)`,
        width: `${this.indicator.w}px`,
        height: `${this.indicator.h}px`,
        opacity: this.indicator.ready ? 1 : 0,
      }
    },
  },
  watch: {
    selected() {
      this.moveIndicator()
    },
    items: {
      handler() {
        this.moveIndicator()
      },
      deep: true,
    },
  },
  created() {
    if (this.items.length > 0 && this.neverEmpty && !this.selected) {
      this.selected = this.items[0]
    }
  },
  mounted() {
    window.addEventListener('resize', this.moveIndicator)
    this.moveIndicator()
  },
  unmounted() {
    window.removeEventListener('resize', this.moveIndicator)
  },
  methods: {
    toggleItem(item) {
      if (this.selected === item && !this.neverEmpty) {
        this.selected = null
      } else {
        this.selected = item
      }
    },
    moveIndicator() {
      this.$nextTick(() => {
        const root = this.$refs.root
        if (!root) return
        const selected = root.querySelector('.btn.selected') || root.querySelector('.btn')
        if (!selected) return
        this.indicator = {
          x: selected.offsetLeft,
          y: selected.offsetTop,
          w: selected.offsetWidth,
          h: selected.offsetHeight,
          ready: true,
        }
      })
    },
  },
})
</script>

<style lang="scss" scoped>
.chips {
  position: relative;
  display: flex;
  grid-gap: 0.5rem;
  flex-wrap: nowrap;
  align-items: center;

  .chip-indicator {
    position: absolute;
    top: 0;
    left: 0;
    height: 2.25rem;
    border-radius: var(--radius-md);
    background: var(--color-brand-highlight);
    pointer-events: none;
    z-index: 0;
    will-change: transform, width;
    transition:
      transform 0.42s cubic-bezier(0.22, 1, 0.36, 1),
      width 0.42s cubic-bezier(0.22, 1, 0.36, 1),
      height 0.42s cubic-bezier(0.22, 1, 0.36, 1),
      opacity 0.25s ease;

    @media (prefers-reduced-motion) {
      transition: none !important;
    }
  }

  .btn {
    position: relative;
    z-index: 1;
    background: transparent;
    box-shadow: none;

    &.capitalize {
      text-transform: capitalize;
    }

    svg {
      width: 1em;
      height: 1em;
    }

    &:focus-visible {
      outline: 0.25rem solid #ea80ff;
      border-radius: 0.25rem;
    }
  }

  .selected {
    color: var(--color-contrast);
    background: transparent;
    box-shadow: none;
  }
}
</style>
