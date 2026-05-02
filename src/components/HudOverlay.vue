<script setup lang="ts">
// HUD root component — placeholder pill for M1.
// M5 will replace this with the full waveform / transcribing / success / error
// states (see doc/plans/02-implementation-roadmap.md M5).
import { useI18n } from "vue-i18n";

const { t } = useI18n();
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
    </div>
  </div>
</template>
