<script setup lang="ts">
// Placeholder for /history. M8 wires this to the Rust-owned SQLite history
// table with paginated infinite scroll + search.
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <section class="space-y-3">
    <h1 class="text-2xl font-semibold tracking-tight">
      {{ t("views.history.heading") }}
    </h1>
    <p class="text-sm text-muted-foreground">
      {{ t("views.history.description") }}
    </p>
  </section>
</template>
