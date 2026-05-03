<script setup lang="ts">
// HUD root view. The HUD window is intentionally minimal — every visual state
// lives inside <HudOverlay> so future milestones can swap visual modes
// without touching the entry point.
//
// M3 chunk 0 add: subscribe to `audio:mic-safety-warning` and `console.error`
// it. M5 will surface this visually inside the HUD; until then we just need
// the warning to land *somewhere* in release builds (the matching Rust
// `eprintln!` is invisible there).
import { onMounted, onUnmounted } from "vue";

import HudOverlay from "@/components/HudOverlay.vue";
import { EVENT_NAMES, listenToEvent } from "@/composables/useTauriEvents";
import type { MicSafetyPayload } from "@/types";

let unlistenMicSafety: (() => void) | null = null;

onMounted(async () => {
  unlistenMicSafety = await listenToEvent<MicSafetyPayload>(
    EVENT_NAMES.AUDIO_MIC_SAFETY_WARNING,
    (event) => {
       
      // this visually; until then console.error is the release-friendly
      // alternative to the Rust-side `eprintln!` (stderr → /dev/null in
      // packaged builds).
      console.error(
        "[hud] audio:mic-safety-warning:",
        event.payload.detail,
      );
    },
  );
});

onUnmounted(() => {
  unlistenMicSafety?.();
  unlistenMicSafety = null;
});
</script>

<template>
  <HudOverlay />
</template>
