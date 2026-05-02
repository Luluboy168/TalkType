<script setup lang="ts">
// Placeholder for /settings. M8 splits this into sub-components per area
// (hotkey / transcription / LLM / audio / API keys / appearance / advanced)
// to keep file size sane (SayIt's monolithic SettingsView hit 1907 lines).
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <section class="space-y-3">
    <h1 class="text-2xl font-semibold tracking-tight">
      {{ t("views.settings.heading") }}
    </h1>
    <p class="text-sm text-muted-foreground">
      {{ t("views.settings.description") }}
    </p>
  </section>
</template>
