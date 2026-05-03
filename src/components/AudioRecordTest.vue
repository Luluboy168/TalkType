<script setup lang="ts">
// Dev-only smoke card for the M2 audio recorder pipeline. Sits below
// <IpcSmokeTest> on the Dashboard so contributors can visually verify the
// FFT waveform + WAV round-trip without booting the full hotkey + voice flow
// machinery (those land in M4 / M5).
//
// Lifecycle:
//   * "Start" invokes `start_recording` (no device → cpal default).
//     `useAudioWaveform.start()` listens to `audio:waveform` and exposes 6
//     smoothed bar levels.
//   * Optional auto-stop toggle: when on, a 3 s timer fires "Stop" once the
//     recording starts. M2 acceptance criterion ("3 s recording yields a
//     valid WAV") rides on this path.
//   * "Stop" invokes `stop_recording`, captures the StopRecordingResult, and
//     stops the waveform composable. The Rust-side WAV bytes stay buffered
//     in `AudioRecorderState::wav_buffer` until "Save WAV" reads them.
//   * "Save WAV" invokes `save_recording_file` (no id → fresh UUID v4) and
//     displays the returned `recordings/<uuid>.wav` relative path.
//
// Removed or hidden behind a debug flag in M9 polish. Kept on /dashboard for
// now so we can manually verify M2 + future M3 transcription.

import { invoke } from "@tauri-apps/api/core";
import { computed, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useAudioWaveform } from "@/composables/useAudioWaveform";
import type { StopRecordingResult } from "@/types";

type RecordingStatus = "idle" | "recording" | "stopped" | "saved" | "error";

const { t } = useI18n();
const { smoothedLevels, start: startWaveform, stop: stopWaveform } =
  useAudioWaveform();

const status = ref<RecordingStatus>("idle");
const elapsedSeconds = ref(0);
const stopResult = ref<StopRecordingResult | null>(null);
const savedPath = ref<string | null>(null);
const errorMessage = ref<string | null>(null);
const autoStopEnabled = ref(true);

let elapsedTimer: ReturnType<typeof setInterval> | null = null;
let autoStopTimer: ReturnType<typeof setTimeout> | null = null;
let recordingStartedAt = 0;

/** Auto-stop window for the M2 acceptance criterion. SayIt uses ~3 s. */
const AUTO_STOP_MS = 3_000;

/** Bar height percentages, computed from smoothed FFT levels (each in [0,1]). */
const barHeights = computed<string[]>(() =>
  smoothedLevels.value.map(
    (level) => `${Math.min(100, Math.max(0, level * 100))}%`,
  ),
);

const statusLabel = computed<string>(() => {
  switch (status.value) {
    case "idle":
      return t("dashboard.audioTest.idle");
    case "recording":
      return `${t("dashboard.audioTest.recording")} (${elapsedSeconds.value}s)`;
    case "stopped": {
      const r = stopResult.value;
      if (!r) return t("dashboard.audioTest.stopped");
      const durSec = (r.durationMs / 1000).toFixed(1);
      return `${t("dashboard.audioTest.stopped")} (${durSec} s, peak=${r.peakEnergyLevel.toFixed(3)}, rms=${r.rmsEnergyLevel.toFixed(3)})`;
    }
    case "saved":
      return savedPath.value
        ? `${t("dashboard.audioTest.saved")} → ${savedPath.value}`
        : t("dashboard.audioTest.saved");
    case "error":
      return `${t("dashboard.audioTest.error")}: ${errorMessage.value ?? ""}`;
    default:
      return "";
  }
});

function clearTimers(): void {
  if (elapsedTimer) {
    clearInterval(elapsedTimer);
    elapsedTimer = null;
  }
  if (autoStopTimer) {
    clearTimeout(autoStopTimer);
    autoStopTimer = null;
  }
}

async function handleStart(): Promise<void> {
  if (status.value === "recording") return;
  errorMessage.value = null;
  stopResult.value = null;
  savedPath.value = null;
  try {
    await invoke<void>("start_recording", { deviceName: null });
    await startWaveform();
    status.value = "recording";
    recordingStartedAt = Date.now();
    elapsedSeconds.value = 0;
    elapsedTimer = setInterval(() => {
      elapsedSeconds.value = Math.floor(
        (Date.now() - recordingStartedAt) / 1000,
      );
    }, 250);
    if (autoStopEnabled.value) {
      autoStopTimer = setTimeout(() => {
        void handleStop();
      }, AUTO_STOP_MS);
    }
  } catch (err) {
    errorMessage.value = err instanceof Error ? err.message : String(err);
    status.value = "error";
    console.error("[audio-test] start_recording failed:", err);
  }
}

async function handleStop(): Promise<void> {
  if (status.value !== "recording") return;
  clearTimers();
  try {
    const result = await invoke<StopRecordingResult>("stop_recording");
    stopWaveform();
    stopResult.value = result;
    status.value = "stopped";
  } catch (err) {
    errorMessage.value = err instanceof Error ? err.message : String(err);
    status.value = "error";
    stopWaveform();
    console.error("[audio-test] stop_recording failed:", err);
  }
}

async function handleSave(): Promise<void> {
  if (status.value !== "stopped") return;
  errorMessage.value = null;
  try {
    const path = await invoke<string>("save_recording_file", { id: null });
    savedPath.value = path;
    status.value = "saved";
  } catch (err) {
    errorMessage.value = err instanceof Error ? err.message : String(err);
    status.value = "error";
    console.error("[audio-test] save_recording_file failed:", err);
  }
}

onUnmounted(() => {
  clearTimers();
  // Best-effort: if the user navigates away while recording, tell Rust to
  // tear the cpal stream down. Composable already drops its listener.
  if (status.value === "recording") {
    void invoke<unknown>("stop_recording").catch((err) => {
      console.warn("[audio-test] stop_recording during unmount:", err);
    });
    stopWaveform();
  }
});
</script>

<template>
  <section class="space-y-3 rounded-lg border border-border bg-card p-4 text-card-foreground">
    <header class="flex items-center justify-between gap-3">
      <h2 class="text-sm font-semibold">
        {{ t("dashboard.audioTest.title") }}
      </h2>
      <label class="flex items-center gap-2 text-xs text-muted-foreground">
        <Switch
          v-model="autoStopEnabled"
          size="sm"
          :aria-label="t('dashboard.audioTest.autoStop3s')"
        />
        {{ t("dashboard.audioTest.autoStop3s") }}
      </label>
    </header>

    <div class="flex items-center gap-2">
      <Button
        size="sm"
        :disabled="status === 'recording'"
        @click="handleStart"
      >
        {{ t("dashboard.audioTest.start") }}
      </Button>
      <Button
        size="sm"
        variant="secondary"
        :disabled="status !== 'recording'"
        @click="handleStop"
      >
        {{ t("dashboard.audioTest.stop") }}
      </Button>
      <Button
        size="sm"
        variant="outline"
        :disabled="status !== 'stopped'"
        @click="handleSave"
      >
        {{ t("dashboard.audioTest.save") }}
      </Button>
    </div>

    <div
      class="flex h-16 items-end gap-1.5"
      role="img"
      :aria-label="t('dashboard.audioTest.title')"
    >
      <div
        v-for="(height, index) in barHeights"
        :key="index"
        class="w-3 rounded-sm bg-primary transition-[height] duration-75 ease-linear"
        :style="{ height }"
      />
    </div>

    <p
      class="text-xs text-muted-foreground"
      aria-live="polite"
    >
      {{ statusLabel }}
    </p>
  </section>
</template>
