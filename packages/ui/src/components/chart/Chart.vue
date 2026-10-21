<template>
  <div class="chart-stub" />
</template>

<script setup>
defineProps({
  name: { type: String, default: "" },
  labels: { type: Array, default: () => [] },
  datasets: { type: Array, default: () => [] },
  colors: { type: Array, default: () => [] },
  suffix: { type: String, default: "" },
});
</script>

<style scoped>
.chart-stub {
  min-height: 8rem;
}
</style>
