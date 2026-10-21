<template>
  <div class="chart-stub" />
</template>

<script setup>
defineProps({
  data: { type: Array, default: () => [] },
  labels: { type: Array, default: () => [] },
  colors: { type: Array, default: () => [] },
  hideLegend: { type: Boolean, default: false },
});
</script>

<style scoped>
.chart-stub {
  min-height: 4rem;
}
</style>
