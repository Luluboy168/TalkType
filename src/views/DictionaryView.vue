<script setup lang="ts">
// Placeholder for /dictionary. M8 adds custom vocabulary CRUD that biases the
// Whisper prompt for better accuracy on user-specific terms.
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <section class="space-y-3">
    <h1 class="text-2xl font-semibold tracking-tight">
      {{ t("views.dictionary.heading") }}
    </h1>
    <p class="text-sm text-muted-foreground">
      {{ t("views.dictionary.description") }}
    </p>
  </section>
</template>
