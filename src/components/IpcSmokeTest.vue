<script setup lang="ts">
// Dev-only smoke test for the dual-window IPC contract introduced in M1.
//
// Click "Send ping" -> `invoke<void>('ping')` -> Rust emits `ipc:pong`
// globally -> both this component (Dashboard) and HudOverlay receive it.
//
// Removed once a richer voice flow exists (M5+) — kept here so contributors
// can verify cross-window IPC is actually wired without booting the full
// hotkey + audio stack.
import { invoke } from "@tauri-apps/api/core";
import { onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import { EVENT_NAMES, listenToEvent } from "@/composables/useTauriEvents";
import type { PongPayload } from "@/types";

const { t } = useI18n();

interface PongRecord {
  source: string;
  timestampMs: number;
}

const MAX_RECENT_PONGS = 5;
const recentPongs = ref<PongRecord[]>([]);
const isSending = ref(false);
const lastError = ref<string | null>(null);

let unlistenPong: (() => void) | null = null;

async function sendPing(): Promise<void> {
  isSending.value = true;
  lastError.value = null;
  try {
    await invoke<void>("ping", { source: "main-window" });
  } catch (err) {
    // Vite-only mode (no Tauri runtime) and any Rust-side serialize/transport
    // errors land here. We surface a short string so contributors see "yes,
    // the click reached the IPC layer".
    lastError.value = err instanceof Error ? err.message : String(err);
    console.error("[ipc-smoke] ping failed:", err);
  } finally {
    isSending.value = false;
  }
}

function recordPong(payload: PongPayload): void {
  recentPongs.value = [
    { source: payload.source, timestampMs: payload.timestampMs },
    ...recentPongs.value,
  ].slice(0, MAX_RECENT_PONGS);
}

function formatTimestamp(timestampMs: number): string {
  return new Date(timestampMs).toLocaleTimeString();
}

onMounted(async () => {
  unlistenPong = await listenToEvent<PongPayload>(EVENT_NAMES.PONG, (event) => {
    recordPong(event.payload);
  });
});

onUnmounted(() => {
  unlistenPong?.();
  unlistenPong = null;
});
</script>

<template>
  <section class="space-y-3 rounded-lg border border-border bg-card p-4 text-card-foreground">
    <header class="flex items-center justify-between gap-3">
      <h2 class="text-sm font-semibold">
        {{ t("dashboard.ipcSmoke.title") }}
      </h2>
      <Button
        size="sm"
        :disabled="isSending"
        @click="sendPing"
      >
        {{ t("dashboard.ipcSmoke.sendPing") }}
      </Button>
    </header>

    <p
      v-if="lastError"
      class="text-xs text-destructive"
    >
      {{ lastError }}
    </p>

    <ul
      v-if="recentPongs.length > 0"
      class="space-y-1 text-xs text-muted-foreground"
    >
      <li
        v-for="pong in recentPongs"
        :key="pong.timestampMs"
        class="flex items-center justify-between gap-3 font-mono"
      >
        <span>{{ pong.source }}</span>
        <span>{{ formatTimestamp(pong.timestampMs) }}</span>
      </li>
    </ul>
    <p
      v-else
      class="text-xs text-muted-foreground"
    >
      {{ t("dashboard.ipcSmoke.waiting") }}
    </p>
  </section>
</template>
