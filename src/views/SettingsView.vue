<script setup lang="ts">
// /settings — Phase 1 layout: Hotkey section (M4 chunk 4) → API key section
// (M3 chunk-1, extracted into `<SettingsApiKeySection>`) → "Audio Input"
// section with device picker + RMS preview (M2). M8 will further split into
// seven per-area sub-components (hotkey / transcription / LLM / audio /
// API keys / appearance / advanced) to avoid SayIt's 1907-line monolith.
//
// Mic preview wiring:
//   * `list_audio_input_devices` invoke on mount → drives <Select>.
//   * "Start Preview" invokes `start_audio_preview` → composable listens to
//     `audio:preview-level` and exposes a smoothed [0..1] level we bind to a
//     bar fill width. "Stop Preview" invokes `stop_audio_preview`.
//   * onBeforeUnmount also invokes `stop_audio_preview` so navigating away
//     from /settings tears the cpal stream down (composable cleans up its
//     listener on unmount, but the cpal stream lives in Rust state and needs
//     explicit invoke).

import { invoke } from "@tauri-apps/api/core";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import SettingsApiKeySection from "@/components/SettingsApiKeySection.vue";
import SettingsHotkeySection from "@/components/SettingsHotkeySection.vue";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useAudioPreview } from "@/composables/useAudioPreview";
import type { AudioInputDeviceInfo } from "@/types";

/** Sentinel string used as the <Select> v-model when the user wants the
 * host's default mic (we pass `device_name = null` to Rust which then resolves
 * the cpal default). reka-ui's Select doesn't accept the empty string as a
 * value, so we use a non-name-collision marker. */
const SYSTEM_DEFAULT = "__system_default__" as const;

const { t } = useI18n();

const devices = ref<AudioInputDeviceInfo[]>([]);
const selectedDevice = ref<string>(SYSTEM_DEFAULT);
const previewActive = ref(false);
const lastError = ref<string | null>(null);
/**
 * Guards against double-invokes during the async invoke + listener
 * register/unregister window. Without this, a fast user double-click on
 * "Start preview" / "Stop preview" can fire two `start_audio_preview`
 * commands and Rust returns `audio preview already running` on the second
 * (M2 retro UX gap #1). Watcher-driven device switches set this too.
 */
const inFlight = ref(false);

const { smoothedLevel, start: startPreviewListener, stop: stopPreviewListener } =
  useAudioPreview();

/** Map the v-model sentinel back to `null` when invoking Rust. */
const resolvedDeviceName = computed<string | null>(() =>
  selectedDevice.value === SYSTEM_DEFAULT ? null : selectedDevice.value,
);

/** Bar width as a percentage string for binding to inline style. */
const levelPercent = computed(
  () => `${Math.min(100, Math.max(0, smoothedLevel.value * 100))}%`,
);

async function loadDevices(): Promise<void> {
  try {
    const list = await invoke<AudioInputDeviceInfo[]>(
      "list_audio_input_devices",
    );
    devices.value = list;
    // Preselect the host default if any; otherwise leave the system-default
    // sentinel so Rust resolves the cpal default at preview / record time.
    const def = list.find((d) => d.isDefault);
    if (def) {
      selectedDevice.value = def.name;
    }
  } catch (err) {
    lastError.value = err instanceof Error ? err.message : String(err);
    console.error("[settings] list_audio_input_devices failed:", err);
  }
}

async function togglePreview(): Promise<void> {
  // Idempotency guard: ignore re-entrant clicks while the previous
  // invoke is still in flight (M2 retro UX gap #1).
  if (inFlight.value) return;
  inFlight.value = true;
  lastError.value = null;
  try {
    if (previewActive.value) {
      try {
        await invoke<void>("stop_audio_preview");
      } catch (err) {
        lastError.value = err instanceof Error ? err.message : String(err);
        console.error("[settings] stop_audio_preview failed:", err);
      } finally {
        stopPreviewListener();
        previewActive.value = false;
      }
      return;
    }
    try {
      await invoke<void>("start_audio_preview", {
        deviceName: resolvedDeviceName.value,
      });
      await startPreviewListener();
      previewActive.value = true;
    } catch (err) {
      lastError.value = err instanceof Error ? err.message : String(err);
      console.error("[settings] start_audio_preview failed:", err);
    }
  } finally {
    inFlight.value = false;
  }
}

/**
 * Watch the dropdown selection: when the user picks a different mic while a
 * preview is running, stop the old preview and (re-)start with the new device.
 * Without this, the old cpal stream stays attached to the old mic and the
 * device picker silently lies to the user (M2 retro UX gap #2).
 *
 * `inFlight` guard: if a togglePreview is already running we skip — by the
 * time it returns, `selectedDevice` has already changed and the next user
 * click will pick up the new value.
 */
watch(selectedDevice, async (next, prev) => {
  if (next === prev) return;
  if (!previewActive.value) return;
  if (inFlight.value) return;
  inFlight.value = true;
  try {
    try {
      await invoke<void>("stop_audio_preview");
    } catch (err) {
      console.warn("[settings] stop_audio_preview during switch:", err);
    }
    stopPreviewListener();
    try {
      await invoke<void>("start_audio_preview", {
        deviceName: resolvedDeviceName.value,
      });
      await startPreviewListener();
      previewActive.value = true;
    } catch (err) {
      lastError.value = err instanceof Error ? err.message : String(err);
      previewActive.value = false;
      console.error("[settings] start_audio_preview after switch failed:", err);
    }
  } finally {
    inFlight.value = false;
  }
});

onMounted(() => {
  void loadDevices();
});

// Composable's own onUnmounted already cancels the RAF + drops the listener,
// but the cpal stream lives in Rust state — so issue a stop invoke if we left
// preview running.
onBeforeUnmount(() => {
  if (previewActive.value) {
    void invoke<void>("stop_audio_preview").catch((err) => {
      console.warn("[settings] stop_audio_preview during unmount:", err);
    });
  }
});
</script>

<template>
  <section class="space-y-6">
    <header class="space-y-2">
      <h1 class="text-2xl font-semibold tracking-tight">
        {{ t("views.settings.heading") }}
      </h1>
      <p class="text-sm text-muted-foreground">
        {{ t("views.settings.description") }}
      </p>
    </header>

    <SettingsHotkeySection />

    <SettingsApiKeySection />

    <section
      class="space-y-3 rounded-lg border border-border bg-card p-4 text-card-foreground"
      aria-labelledby="audio-input-heading"
    >
      <h2
        id="audio-input-heading"
        class="text-sm font-semibold"
      >
        {{ t("views.settings.audioInput.title") }}
      </h2>

      <div class="space-y-2">
        <label
          for="mic-device"
          class="text-xs font-medium text-muted-foreground"
        >
          {{ t("views.settings.audioInput.deviceLabel") }}
        </label>
        <div class="flex items-center gap-3">
          <Select v-model="selectedDevice">
            <SelectTrigger
              id="mic-device"
              class="w-72"
            >
              <SelectValue
                :placeholder="t('views.settings.audioInput.systemDefault')"
              />
            </SelectTrigger>
            <SelectContent>
              <SelectItem :value="SYSTEM_DEFAULT">
                {{ t("views.settings.audioInput.systemDefault") }}
              </SelectItem>
              <SelectItem
                v-for="device in devices"
                :key="device.name"
                :value="device.name"
              >
                {{ device.name }}
              </SelectItem>
            </SelectContent>
          </Select>
          <Button
            size="sm"
            :variant="previewActive ? 'secondary' : 'default'"
            @click="togglePreview"
          >
            {{
              previewActive
                ? t("views.settings.audioInput.previewStop")
                : t("views.settings.audioInput.previewStart")
            }}
          </Button>
        </div>
      </div>

      <div class="space-y-1">
        <span
          class="text-xs font-medium text-muted-foreground"
          aria-hidden="true"
        >
          {{ t("views.settings.audioInput.rmsLabel") }}
        </span>
        <div
          class="h-2 w-full overflow-hidden rounded bg-muted"
          role="meter"
          :aria-valuenow="Math.round(smoothedLevel * 100)"
          aria-valuemin="0"
          aria-valuemax="100"
          :aria-label="t('views.settings.audioInput.rmsLabel')"
        >
          <div
            class="h-full bg-primary transition-[width] duration-75 ease-linear"
            :style="{ width: levelPercent }"
          />
        </div>
      </div>

      <p
        v-if="lastError"
        class="text-xs text-destructive"
      >
        {{ lastError }}
      </p>
    </section>
  </section>
</template>
