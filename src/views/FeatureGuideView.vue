<script setup lang="ts">
// Placeholder for /guide. M8 adds the static guide page covering the core
// hold-to-speak workflow, hotkey customization, and provider setup.
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <section class="space-y-3">
    <h1 class="text-2xl font-semibold tracking-tight">
      {{ t("views.guide.heading") }}
    </h1>
    <p class="text-sm text-muted-foreground">
      {{ t("views.guide.description") }}
    </p>
  </section>
</template>
