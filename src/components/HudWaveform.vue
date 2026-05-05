<script setup lang="ts">
// HudWaveform — 6-bar audio visualizer (M5 chunk 2).
//
// Reads frame-smoothed levels from `useAudioWaveform` (M2). When
// `prefers-reduced-motion: reduce` is on (passed in via `reducedMotion` prop),
// renders a single static dot AND does not start the listener / RAF loop —
// full energy-conscious ARIA fallback per spec §5.2 (chunk 2 reviewer P2-1).
//
// All decorative — `aria-hidden="true"` so the SR doesn't announce it.
import { onMounted, onUnmounted, watch } from "vue";

import { useAudioWaveform } from "@/composables/useAudioWaveform";

const props = defineProps<{ reducedMotion: boolean }>();

const { smoothedLevels, start, stop } = useAudioWaveform();

onMounted(() => {
  if (!props.reducedMotion) void start();
});
// React to runtime reduced-motion toggle (system setting changed mid-session)
watch(
  () => props.reducedMotion,
  (rm) => {
    if (rm) stop();
    else void start();
  },
);
onUnmounted(stop);
</script>

<template>
  <div
    v-if="!reducedMotion"
    class="flex items-center gap-1"
    aria-hidden="true"
  >
    <span
      v-for="(level, i) in smoothedLevels"
      :key="i"
      class="w-1 rounded-full bg-primary"
      :style="{ height: `${4 + level * 16}px` }"
    />
  </div>
  <div
    v-else
    aria-hidden="true"
    class="flex items-center gap-2"
  >
    <span class="size-2 rounded-full bg-primary" />
  </div>
</template>
