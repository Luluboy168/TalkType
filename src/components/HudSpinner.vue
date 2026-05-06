<script setup lang="ts">
// HudSpinner — transcribing-state spinner (M5 chunk 2).
//
// Default: small CSS-animated ring (Tailwind `animate-spin`).
// `prefers-reduced-motion: reduce` (via `reducedMotion` prop): static "..."
// glyph so no animation runs at all.
//
// Decorative — `aria-hidden="true"`. The HUD root carries the SR-readable
// label; this component just provides the visual cue.
defineProps<{ reducedMotion: boolean }>();
</script>

<template>
  <span
    v-if="!reducedMotion"
    aria-hidden="true"
    class="inline-block size-4 animate-spin rounded-full border-2 border-primary border-t-transparent"
  />
  <span
    v-else
    aria-hidden="true"
    class="text-sm font-mono"
  >…</span>
</template>
