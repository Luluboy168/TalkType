<script setup lang="ts">
// Placeholder for /dashboard. M8 fills in the overview cards (recent
// transcriptions, today's stats, etc.). M1 embeds the dev-only
// <IpcSmokeTest> so contributors can verify the dual-window IPC contract.
import { useI18n } from "vue-i18n";

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
  </section>
</template>
