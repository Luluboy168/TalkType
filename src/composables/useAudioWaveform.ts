// useAudioWaveform — listens to `audio:waveform` and exposes 6 frame-smoothed
// FFT levels suitable for binding to a 6-bar visualizer.
//
// Pattern: requestAnimationFrame + lerp (per-bar).
//   * Rust emits `audio:waveform` ~60 fps during recording (16 ms tick).
//   * Per-frame: each bar's smoothed value moves toward its target by
//     `(target - smoothed) * 0.25`. The slightly higher alpha than
//     `useAudioPreview` (0.2) keeps bar response feeling crisp during speech.
//
// As with useAudioPreview, the composable does NOT call `start_recording` —
// the parent component / store owns that. This composable just manages the
// listener + animation lifecycle.

import { onUnmounted, ref } from "vue";

import {
  AUDIO_WAVEFORM,
  listenToEvent,
} from "./useTauriEvents";

import type { WaveformPayload } from "@/types";

/** Number of waveform bars exposed; mirrors `WAVEFORM_BIN_COUNT` in Rust. */
const BAR_COUNT = 6;

/** Lerp factor applied per animation frame. 0.25 → reaches target in ~4 frames. */
const LERP_ALPHA = 0.25;

export function useAudioWaveform() {
  const smoothedLevels = ref<number[]>(new Array<number>(BAR_COUNT).fill(0));
  const targetLevels: number[] = new Array<number>(BAR_COUNT).fill(0);
  let raf = 0;
  let unlisten: (() => void) | null = null;
  // P1-3 (M5 chunk 2): rapid hotkey press could mount → start() in flight
  // → unmount before the await listenToEvent resolves → stop() finds no
  // unlisten yet → second mount races and creates a duplicate listener.
  // The `starting` flag short-circuits re-entry while the first start is
  // still awaiting registration.
  let starting = false;

  function tick() {
    const next = smoothedLevels.value.slice();
    for (let i = 0; i < BAR_COUNT; i++) {
      next[i] = next[i] + (targetLevels[i] - next[i]) * LERP_ALPHA;
    }
    smoothedLevels.value = next;
    raf = requestAnimationFrame(tick);
  }

  async function start() {
    if (unlisten || starting) return;
    starting = true;
    try {
      unlisten = await listenToEvent<WaveformPayload>(
        AUDIO_WAVEFORM,
        (event) => {
          const { levels } = event.payload;
          for (let i = 0; i < BAR_COUNT; i++) {
            targetLevels[i] = levels[i] ?? 0;
          }
        },
      );
      raf = requestAnimationFrame(tick);
    } finally {
      starting = false;
    }
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
    for (let i = 0; i < BAR_COUNT; i++) {
      targetLevels[i] = 0;
    }
    smoothedLevels.value = new Array<number>(BAR_COUNT).fill(0);
  }

  onUnmounted(stop);

  return { smoothedLevels, start, stop };
}
