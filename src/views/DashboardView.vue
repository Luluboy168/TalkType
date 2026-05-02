<script setup lang="ts">
// Placeholder for /dashboard. M8 fills in the overview cards (recent
// transcriptions, today's stats, etc.).
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <section class="space-y-3">
    <h1 class="text-2xl font-semibold tracking-tight">
      {{ t("views.dashboard.heading") }}
    </h1>
    <p class="text-sm text-muted-foreground">
      {{ t("views.dashboard.description") }}
    </p>
  </section>
</template>
