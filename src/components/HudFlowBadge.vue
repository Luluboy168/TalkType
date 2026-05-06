<script setup lang="ts">
// Dashboard sidebar recording indicator (M5 chunk 3).
//
// Listens to the cross-window `voice-flow:state-changed` event broadcast
// by the HUD-side `useVoiceFlowStore` and renders a red pulsing dot +
// 「錄音中」 label only when the active status is `recording`. All other
// states render nothing — badge presence is the signal.
//
// Echo-loop defense (challenger P0-1 / P1-7): the payload carries a
// `source` discriminator and we ignore anything that did not originate
// from the HUD. Phase 1 the Dashboard never emits this event today, but
// this guard is cheap insurance for future symmetry.
//
// Per CLAUDE.md "唯一 import @tauri-apps/api/event 的地方" rule, we go
// through `listenToEvent` from `@/composables/useTauriEvents` instead
// of importing `listen` directly.
import { onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  listenToEvent,
  VOICE_FLOW_STATE_CHANGED,
} from "@/composables/useTauriEvents";
import type { VoiceFlowStateChangedPayload } from "@/types/events";

const { t } = useI18n();

type VoiceFlowStatus = VoiceFlowStateChangedPayload["status"];

const status = ref<VoiceFlowStatus>("idle");
let unlisten: (() => void) | null = null;

onMounted(async () => {
  unlisten = await listenToEvent<VoiceFlowStateChangedPayload>(
    VOICE_FLOW_STATE_CHANGED,
    (event) => {
      // Echo-loop defense: ignore any event we (Dashboard) might emit
      // ourselves in the future. Today only the HUD emits, but staying
      // strict here keeps the contract simple as features grow.
      if (event.payload.source !== "hud") return;
      status.value = event.payload.status;
    },
  );
});

onUnmounted(() => {
  unlisten?.();
  unlisten = null;
});
</script>

<template>
  <div
    v-if="status === 'recording'"
    class="flex items-center gap-2 px-3 py-2"
    role="status"
    aria-live="polite"
  >
    <span
      class="size-2 animate-pulse rounded-full bg-red-500"
      aria-hidden="true"
    />
    <span class="text-xs text-foreground">{{ t("sidebar.recordingBadge") }}</span>
  </div>
</template>
