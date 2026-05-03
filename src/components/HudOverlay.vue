<script setup lang="ts">
// HUD root component — placeholder pill for M1.
// M5 will replace this with the full waveform / transcribing / success / error
// states (see doc/plans/02-implementation-roadmap.md M5).
//
// M1 dev-only addition: a tiny pong counter that listens for `ipc:pong` so the
// dual-window IPC contract is observably working from the HUD side. Removed
// once the real voice-flow store consumes events directly.
import { onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { EVENT_NAMES, listenToEvent } from "@/composables/useTauriEvents";
import type { PongPayload } from "@/types";

const { t } = useI18n();

const pongCount = ref(0);
let unlistenPong: (() => void) | null = null;

onMounted(async () => {
  unlistenPong = await listenToEvent<PongPayload>(EVENT_NAMES.PONG, () => {
    pongCount.value += 1;
  });
});

onUnmounted(() => {
  unlistenPong?.();
  unlistenPong = null;
});
</script>

<template>
  <div class="flex h-screen w-screen items-center justify-center">
    <div
      class="flex items-center gap-3 rounded-full border border-border bg-card px-5 py-2.5 text-card-foreground shadow-sm"
      role="status"
      aria-live="polite"
    >
      <span
        aria-hidden="true"
        class="inline-block size-2.5 rounded-full bg-primary"
      />
      <span class="text-sm font-medium">{{ t("hud.title") }}</span>
      <span
        v-if="pongCount > 0"
        class="font-mono text-xs text-muted-foreground"
        :aria-label="t('hud.pongCount', { count: pongCount })"
      >
        {{ t("hud.pongCount", { count: pongCount }) }}
      </span>
    </div>
  </div>
</template>
