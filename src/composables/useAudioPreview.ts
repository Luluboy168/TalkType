// useAudioPreview — listens to `audio:preview-level` and exposes a
// frame-smoothed level for binding to UI bars / meters.
//
// Pattern: requestAnimationFrame + lerp.
//   * Rust emits `audio:preview-level` ~33 fps (30 ms tick).
//   * The UI prefers ~60 fps for visual smoothness.
//   * Each animation frame, smoothed = smoothed + (target - smoothed) * 0.2.
//     0.2 is fast enough to track the mic but slow enough to mask jitter.
//
// The composable does NOT call `start_audio_preview` itself — that's a
// `invoke()` the consumer (the Settings mic picker view) makes explicitly so
// it can show error toasts. This composable only manages the listener +
// animation lifecycle.
//
// Imports `listenToEvent` from `useTauriEvents` per
// `doc/plans/01-architecture.md` invariant rule #3 (only `useTauriEvents.ts`
// imports `@tauri-apps/api/event` directly).

import { onUnmounted, ref } from "vue";

import {
  AUDIO_PREVIEW_LEVEL,
  listenToEvent,
} from "./useTauriEvents";

import type { AudioPreviewLevelPayload } from "@/types";

/** Lerp factor applied per animation frame. 0.2 → reaches target in ~5 frames. */
const LERP_ALPHA = 0.2;

export function useAudioPreview() {
  const smoothedLevel = ref(0);
  let target = 0;
  let raf = 0;
  let unlisten: (() => void) | null = null;

  function tick() {
    smoothedLevel.value =
      smoothedLevel.value + (target - smoothedLevel.value) * LERP_ALPHA;
    raf = requestAnimationFrame(tick);
  }

  async function start() {
    if (unlisten) return;
    unlisten = await listenToEvent<AudioPreviewLevelPayload>(
      AUDIO_PREVIEW_LEVEL,
      (event) => {
        target = event.payload.level;
      },
    );
    raf = requestAnimationFrame(tick);
  }

  function stop() {
    if (raf) {
      cancelAnimationFrame(raf);
      raf = 0;
    }
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
    target = 0;
    smoothedLevel.value = 0;
  }

  onUnmounted(stop);

  return { smoothedLevel, start, stop };
}
