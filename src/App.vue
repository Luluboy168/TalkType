<script setup lang="ts">
// HUD root view. The HUD window is intentionally minimal — every state lives
// inside <HudOverlay> so future milestones can swap visual modes without
// touching the entry point.
import HudOverlay from "@/components/HudOverlay.vue";
</script>

<template>
  <HudOverlay />
</template>
