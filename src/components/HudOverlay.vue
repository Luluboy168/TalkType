<script setup lang="ts">
// HudOverlay — root HUD bubble orchestrator (M5 chunk 2 rewrite).
//
// Replaces the M1 placeholder pong counter with the real 4-state machine:
// recording / transcribing / success / error. Each state mounts a different
// inner bubble inside `<Transition mode="out-in">`; the root container
// carries the ARIA semantics + click-to-dismiss handler + click-through
// toggle.
//
// Reactive system:
//   * status / message / recordingStartedAtMs come from `useVoiceFlowStore`
//     (M5 chunk 1; readonly refs).
//   * `prefers-reduced-motion: reduce` reactivity via `window.matchMedia`
//     + 'change' listener — picks `instant` transition variant + cascades
//     to HudWaveform / HudSpinner so they render their static fallbacks
//     without running any RAF / CSS animation.
//   * `setIgnoreCursorEvents` is toggled in `watch(status)`:
//       - 'error' → false (so the user can click to dismiss)
//       - any other → true (HUD is purely decorative)
//
// Click semantics: only act when status is `error` — for other states we
// shouldn't be clickable anyway because click-through is on, but the guard
// defends against any race where the watcher is mid-await.
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CheckCircle2, XCircle } from "lucide-vue-next";

import HudSpinner from "@/components/HudSpinner.vue";
import HudTimer from "@/components/HudTimer.vue";
import HudWaveform from "@/components/HudWaveform.vue";
import { useVoiceFlowStore } from "@/stores/useVoiceFlowStore";

const store = useVoiceFlowStore();
const { t } = useI18n();

const reducedMotion = ref(false);
let mediaQuery: MediaQueryList | null = null;

const visible = computed(() => store.status !== "idle");
const ariaMessage = computed(() => {
  switch (store.status) {
    case "recording":
      return t("hud.aria.recording");
    case "transcribing":
      return t("hud.aria.transcribing");
    case "success":
      return t("hud.aria.success");
    case "error":
      return t("hud.aria.error", { message: store.message });
    default:
      return "";
  }
});
const transitionName = computed(() =>
  reducedMotion.value ? "instant" : "fade",
);

async function syncClickThrough(status: string): Promise<void> {
  // Keep window click-through-on for everything except error; the user must
  // be able to click the bubble to dismiss the error state.
  await getCurrentWindow().setIgnoreCursorEvents(status !== "error");
}

watch(
  () => store.status,
  (next) => {
    void syncClickThrough(next);
  },
);

function handleHudClick(): void {
  if (store.status === "error") store.dismissError();
}

function handleMediaChange(e: MediaQueryListEvent): void {
  reducedMotion.value = e.matches;
}

onMounted(async () => {
  mediaQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
  reducedMotion.value = mediaQuery.matches;
  mediaQuery.addEventListener("change", handleMediaChange);
  await getCurrentWindow().setIgnoreCursorEvents(true);
});

onUnmounted(() => {
  mediaQuery?.removeEventListener("change", handleMediaChange);
  mediaQuery = null;
});
</script>

<template>
  <div
    v-if="visible"
    class="flex h-screen w-screen items-center justify-center"
    role="status"
    aria-live="polite"
    :aria-label="ariaMessage"
    @click="handleHudClick"
  >
    <Transition
      :name="transitionName"
      mode="out-in"
    >
      <div
        v-if="store.status === 'recording'"
        key="recording"
        class="bubble"
      >
        <HudWaveform :reduced-motion="reducedMotion" />
        <HudTimer :started-at-ms="store.recordingStartedAtMs" />
      </div>
      <div
        v-else-if="store.status === 'transcribing'"
        key="transcribing"
        class="bubble"
      >
        <HudSpinner :reduced-motion="reducedMotion" />
        <span class="bubble-label">{{ t("hud.transcribing") }}</span>
      </div>
      <div
        v-else-if="store.status === 'success'"
        key="success"
        class="bubble"
      >
        <CheckCircle2
          :size="20"
          aria-hidden="true"
          class="text-green-600"
        />
        <span class="bubble-label">{{ t("hud.success") }}</span>
      </div>
      <div
        v-else-if="store.status === 'error'"
        key="error"
        class="bubble"
      >
        <XCircle
          :size="20"
          aria-hidden="true"
          class="text-red-600"
        />
        <span class="bubble-label">{{ store.message }}</span>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.bubble {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  border-radius: 9999px;
  background: var(--card);
  color: var(--card-foreground);
  border: 1px solid var(--border);
  padding: 0.5rem 1rem;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.bubble-label {
  font-size: 0.875rem;
  font-weight: 500;
  /* P1-1 (M5 chunk 2): bubble width 380px max — long Rust error strings
     must truncate with ellipsis instead of overflowing the HUD. */
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 280px;
}

/* fade transition (200ms) — default */
.fade-enter-active,
.fade-leave-active {
  transition:
    opacity 200ms ease,
    transform 200ms ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
  transform: scale(0.95);
}
.fade-enter-to,
.fade-leave-from {
  opacity: 1;
  transform: scale(1);
}

/* instant transition (prefers-reduced-motion: reduce) */
.instant-enter-active,
.instant-leave-active {
  transition: none;
}
</style>
