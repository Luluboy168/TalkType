<script setup lang="ts">
// Placeholder for /dashboard. M8 fills in the overview cards (recent
// transcriptions, today's stats, etc.). M1 embeds <IpcSmokeTest>; M2 also
// embeds <AudioRecordTest> so contributors can verify the cpal recorder +
// FFT waveform path without booting the full hotkey + voice flow stack.
// Both cards are dev-only and will be retired (or hidden behind a debug
// flag) in M9 polish.
import { useI18n } from "vue-i18n";

import AudioRecordTest from "@/components/AudioRecordTest.vue";
import IpcSmokeTest from "@/components/IpcSmokeTest.vue";

const { t } = useI18n();
</script>

<template>
  <section class="space-y-6">
    <header class="space-y-2">
      <h1 class="text-2xl font-semibold tracking-tight">
        {{ t("views.dashboard.heading") }}
      </h1>
      <p class="text-sm text-muted-foreground">
        {{ t("views.dashboard.description") }}
      </p>
    </header>
    <IpcSmokeTest />
    <AudioRecordTest />
  </section>
</template>
