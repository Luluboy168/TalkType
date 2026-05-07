<script setup lang="ts">
// HudOverlay — root HUD bubble orchestrator (M5 chunk 2 rewrite, M6 chunk 3).
//
// **M5 baseline**: 4 visual states recording / transcribing / success / error,
// each mounted in `<Transition mode="out-in">`; root container carries ARIA
// semantics + click-to-dismiss + click-through toggle.
//
// **M6 chunk 3** adds:
//   * 5th `enhancing` state between transcribing and success — HudSpinner +
//     「優化中…」 label (Decision #5 the visual state count grew by one).
//   * Success bubble dual-mode (Decision #5): when `store.polishWarning ===
//     true` the success bubble swaps `CheckCircle2` (green-600) for
//     `AlertTriangle` (amber-500) and shows the「優化失敗、已貼上原始轉錄」
//     warning string. This keeps the user informed that polish silently
//     fell back without throwing them into the red error state (raw paste
//     still ran successfully — degraded-but-OK).
//   * F27 click-through: `enhancing` is ON (same as transcribing / success).
//     Only `error` remains OFF so the user can click to dismiss.
//   * F28 ARIA differentiation: `hud.aria.transcribing` ("轉錄中") vs
//     `hud.aria.enhancing` ("優化中") differ by 2+ Unicode characters so
//     screen readers announce both transitions distinctly.
//
// Reactive system:
//   * status / message / recordingStartedAtMs / polishWarning come from
//     `useVoiceFlowStore` (M5 chunk 1; readonly refs); `polishWarning` was
//     added in M6 chunk 2.
//   * `prefers-reduced-motion: reduce` reactivity via `window.matchMedia`
//     + 'change' listener — picks `instant` transition variant + cascades
//     to HudWaveform / HudSpinner so they render their static fallbacks
//     without running any RAF / CSS animation.
//   * `setIgnoreCursorEvents` is toggled in `watch(status)`:
//       - 'error' → false (so the user can click to dismiss)
//       - any other (recording / transcribing / enhancing / success) → true
//
// Click semantics: only act when status is `error` — for other states we
// shouldn't be clickable anyway because click-through is on, but the guard
// defends against any race where the watcher is mid-await.
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AlertTriangle, CheckCircle2, XCircle } from "lucide-vue-next";

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
    case "enhancing":
      // F28: distinct wording from `transcribing` so SR announces both
      // transitions (tested via Unicode-aware \p{L} comparison in
      // src/__tests__/aria-wording.test.ts).
      return t("hud.aria.enhancing");
    case "success":
      // Decision #5 success bubble dual-mode — when polish fell back to
      // raw paste, replace「完成」with the polish-failed warning so the
      // SR matches the visual amber AlertTriangle.
      return store.polishWarning
        ? t("hud.warning.polishFailed")
        : t("hud.aria.success");
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
  // F27 (M6 chunk 3): the new `enhancing` state is click-through ON (same
  // as transcribing / success) — only `error` remains OFF. The boolean
  // expression `status !== "error"` covers all 5 visual states correctly
  // without needing an explicit case for `enhancing`.
  // Surface failures (e.g. capability missing) so they don't fail silently —
  // M5 acceptance found that without core:window:allow-set-ignore-cursor-events
  // the IPC call rejects and HUD blocks all clicks.
  try {
    await getCurrentWindow().setIgnoreCursorEvents(status !== "error");
  } catch (err) {
    console.error("[hud] setIgnoreCursorEvents failed", err);
  }
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
  // Surface any capability / IPC error so M5 acceptance bugs don't go silent.
  try {
    await getCurrentWindow().setIgnoreCursorEvents(true);
  } catch (err) {
    console.error("[hud] initial setIgnoreCursorEvents(true) failed", err);
  }
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
        v-else-if="store.status === 'enhancing'"
        key="enhancing"
        class="bubble"
      >
        <HudSpinner :reduced-motion="reducedMotion" />
        <span class="bubble-label">{{ t("hud.enhancing") }}</span>
      </div>
      <div
        v-else-if="store.status === 'success'"
        key="success"
        class="bubble"
      >
        <!-- Decision #5 dual-mode: amber AlertTriangle + warning label when
             polish fell back, else green CheckCircle2 + 「完成」 (M5 baseline). -->
        <AlertTriangle
          v-if="store.polishWarning"
          :size="20"
          aria-hidden="true"
          class="text-amber-500"
        />
        <CheckCircle2
          v-else
          :size="20"
          aria-hidden="true"
          class="text-green-600"
        />
        <span class="bubble-label">
          {{
            store.polishWarning
              ? t("hud.warning.polishFailed")
              : t("hud.success")
          }}
        </span>
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
  /* Defense-in-depth: prevent text selection inside HUD even if click-through
     fails. User reported during M5 acceptance that the timer was 反白-able,
     which signals a click-through gap; user-select:none kills that vector. */
  user-select: none;
  -webkit-user-select: none;
  cursor: default;
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
