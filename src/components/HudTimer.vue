<script setup lang="ts">
// HudTimer — running mm:ss elapsed display (M5 chunk 2).
//
// Color thresholds per spec §2 + §5:
//   * < 9 min   → muted-foreground
//   * 9–12 min  → yellow-600 (cap warning)
//   * ≥ 12 min  → red-600 (very close to ~13 min size cap auto-abort)
//
// P1-5 regression: `startedAtMs: null` → renders "0:00" with no errors. The
// computed `elapsed` short-circuits to 0 when null; the interval still runs
// (cheap) but produces no observable update because `props.startedAtMs` stays
// null.
//
// Cleanup: setInterval cleared on unmount. Uses `ReturnType<typeof setInterval>`
// to satisfy both Node + browser typings without env-specific imports.
import { computed, onMounted, onUnmounted, ref } from "vue";

const props = defineProps<{ startedAtMs: number | null }>();

const now = ref(Date.now());
let interval: ReturnType<typeof setInterval> | null = null;

const elapsed = computed(() => {
  if (!props.startedAtMs) return 0;
  return Math.max(0, Math.floor((now.value - props.startedAtMs) / 1000));
});

const formatted = computed(() => {
  const m = Math.floor(elapsed.value / 60);
  const s = elapsed.value % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
});

const colorClass = computed(() => {
  // Order matters: ≥720 must be checked before ≥540 so the 12-min threshold
  // wins over the 9-min one for elapsed values in both ranges.
  if (elapsed.value >= 720) return "text-red-600";
  if (elapsed.value >= 540) return "text-yellow-600";
  return "text-muted-foreground";
});

onMounted(() => {
  interval = setInterval(() => {
    now.value = Date.now();
  }, 1000);
});

onUnmounted(() => {
  if (interval) clearInterval(interval);
  interval = null;
});
</script>

<template>
  <span
    class="font-mono text-sm"
    :class="colorClass"
  >{{ formatted }}</span>
</template>
