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
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useAudioWaveform } from "@/composables/useAudioWaveform";
import type { StopRecordingResult, TranscriptionResult } from "@/types";

type RecordingStatus =
  | "idle"
  | "recording"
  | "stopped"
  | "transcribing"
  | "transcribed"
  | "saved"
  | "error";

const { t } = useI18n();
const { smoothedLevels, start: startWaveform, stop: stopWaveform } =
  useAudioWaveform();

const status = ref<RecordingStatus>("idle");
const elapsedSeconds = ref(0);
const stopResult = ref<StopRecordingResult | null>(null);
const savedPath = ref<string | null>(null);
const errorMessage = ref<string | null>(null);
const autoStopEnabled = ref(true);

// ─── Transcription smoke (M3 chunk 3) ─────────────────────────────────────

/** Result of the most recent `transcribe_audio` call. `null` when no
 * transcription has run yet (or after the user clears it via Start). */
const transcriptionResult = ref<TranscriptionResult | null>(null);
/** Whether a Groq API key is currently saved — gates the "Test Transcribe"
 * button. Refreshed on mount and whenever the recorder transitions back to
 * `stopped` (so a key set mid-session shows up without a remount). */
const hasGroqKey = ref<boolean>(false);

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
    case "transcribing":
      return t("dashboard.audioTest.transcribing");
    case "transcribed":
      return t("dashboard.audioTest.transcribed");
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
  transcriptionResult.value = null;
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

/** Maps Rust `TranscriptionError` flat-strings to localized messages.
 * Mirrors the `formatTestError` pattern from `SettingsApiKeySection.vue`,
 * but for the chunk-2 `TranscriptionError` enum — different prefixes
 * (e.g. `"Audio too small to transcribe"`, `"A previous transcription is
 * still in progress"`) so we can't reuse the test-connection mapping. */
function formatTranscribeError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  if (raw.startsWith("API key missing")) {
    return t("dashboard.audioTest.transcribeError.noKey");
  }
  if (raw === "A previous transcription is still in progress") {
    return t("dashboard.audioTest.transcribeError.busy");
  }
  if (raw.startsWith("Audio too small")) {
    return t("dashboard.audioTest.transcribeError.tooSmall");
  }
  if (raw.startsWith("Audio exceeds Groq")) {
    return t("dashboard.audioTest.transcribeError.tooLarge");
  }
  if (
    raw.startsWith("Network appears offline") ||
    raw.startsWith("DNS lookup failed") ||
    raw.startsWith("TLS handshake failed") ||
    raw.startsWith("Request timed out") ||
    raw.startsWith("Connection refused") ||
    raw.startsWith("Other network error")
  ) {
    return t("dashboard.audioTest.transcribeError.network");
  }
  return t("dashboard.audioTest.transcribeError.unknown", { detail: raw });
}

async function refreshHasGroqKey(): Promise<void> {
  try {
    hasGroqKey.value = await invoke<boolean>("has_credential", {
      provider: "groq",
    });
  } catch (err) {
    // Don't surface — just leave the button disabled if we can't check.
    console.warn("[audio-test] has_credential check failed:", err);
    hasGroqKey.value = false;
  }
}

async function handleTranscribe(): Promise<void> {
  if (status.value !== "stopped") return;
  errorMessage.value = null;
  transcriptionResult.value = null;
  status.value = "transcribing";
  try {
    // M3 ships only Groq cloud; vocabulary is null for the smoke surface
    // (M8's Dictionary view will populate it once history persistence
    // arrives). The Rust dispatcher consumes the WAV buffer on success;
    // returning the recorder to "stopped" after error keeps the buffer
    // so the user can retry without re-recording.
    const result = await invoke<TranscriptionResult>("transcribe_audio", {
      vocabulary: null,
    });
    transcriptionResult.value = result;
    status.value = "transcribed";
  } catch (err) {
    errorMessage.value = formatTranscribeError(err);
    // Stay in "stopped" so the user can retry; a fresh "Start" still resets
    // the result + error fields (see handleStart). For Busy / non-recoverable
    // cases the Rust side has already preserved the buffer state anyway.
    status.value = "stopped";
    console.error("[audio-test] transcribe_audio failed:", err);
  }
}

onMounted(() => {
  // Settings can land here with a key already saved or absent; refresh on
  // every mount so the "Test Transcribe" button reflects the current state.
  void refreshHasGroqKey();
});

// Re-check has_credential when the recorder reaches a stoppable transcription
// point. User may have set the key in Settings between mount and stop, so
// gating only on the onMount check would miss that flow.
watch(status, (next) => {
  if (next === "stopped") {
    void refreshHasGroqKey();
  }
});

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
  // M3 chunk 0 (M2 retro #3): if the user stopped recording but never saved,
  // the WAV bytes are still sitting in `AudioRecorderState::wav_buffer` —
  // ask Rust to drop them so we don't keep ~50 MB pinned across navigation.
  // Best-effort: don't await, don't escalate failures (no cleanup is fine).
  if (status.value === "stopped" || status.value === "transcribed") {
    void invoke<unknown>("clear_recording_buffer").catch((err) => {
      console.warn("[audio-test] clear_recording_buffer during unmount:", err);
    });
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

    <div class="flex flex-wrap items-center gap-2">
      <Button
        size="sm"
        :disabled="status === 'recording' || status === 'transcribing'"
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
      <Button
        size="sm"
        variant="outline"
        :disabled="status !== 'stopped' || !hasGroqKey"
        @click="handleTranscribe"
      >
        {{
          status === "transcribing"
            ? t("dashboard.audioTest.transcribing")
            : t("dashboard.audioTest.transcribe")
        }}
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

    <div
      v-if="transcriptionResult"
      class="space-y-1 rounded-md bg-muted px-3 py-2"
    >
      <p class="text-xs font-medium text-muted-foreground">
        {{ t("dashboard.audioTest.resultLabel") }}
      </p>
      <textarea
        :value="transcriptionResult.rawText"
        readonly
        rows="3"
        class="w-full resize-none rounded border border-border bg-background p-2 font-mono text-xs text-foreground"
        :aria-label="t('dashboard.audioTest.resultLabel')"
      />
      <p class="text-xs text-muted-foreground">
        {{
          t("dashboard.audioTest.resultDuration", {
            ms: transcriptionResult.transcriptionDurationMs,
          })
        }}<span v-if="transcriptionResult.noSpeechProbability !== null">
          ·
          {{
            t("dashboard.audioTest.resultNoSpeechProb", {
              prob: transcriptionResult.noSpeechProbability.toFixed(3),
            })
          }}
        </span>
      </p>
    </div>
  </section>
</template>
