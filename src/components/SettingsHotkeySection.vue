<script setup lang="ts">
// Settings → Hotkey section (M4 chunk 4).
//
// Lets the user pick:
//   * Trigger key (preset dropdown — Right/Left Alt, Ctrl, Shift)
//   * Trigger mode (RadioGroup — Hold / Toggle)
//
// Phase 1 ships preset-only — `Custom { keycode }` and `Combo { ... }` are
// deferred to Phase 2 (per `doc/plans/02-implementation-roadmap.md` M4 task
// "不做 Custom keycode recording — preset only — 簡化"). The
// `start_hotkey_recording` / `cancel_hotkey_recording` Tauri commands are
// stubs that this section does NOT call.
//
// **Persistence path**: dropdown / radio mutate a local `draft` ref; clicking
// "Save" invokes `update_settings({ hotkey })` via the store. The Rust side
// hot-swaps the live `HotkeyListenerState` atomics in the same transaction
// (see `settings.rs::SettingsState::update`) and broadcasts
// `settings:updated`. The store's listener then refreshes the cached
// snapshot, which propagates through `syncDraftFromSettings` so the form
// reflects the saved state.
//
// **Mirror the API key section**: same shape (`<Card>` containing `<Select>`
// + `<RadioGroup>` + `<Button>`), same i18n key style, same onMounted /
// onUnmounted timer cleanup pattern. See `SettingsApiKeySection.vue`.
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Loader2 } from "lucide-vue-next";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useSettingsStore } from "@/stores/useSettingsStore";
import type { HotkeyConfig, TriggerKey, TriggerMode } from "@/types/settings";

const { t } = useI18n();
const settingsStore = useSettingsStore();

// ─── Local draft state ────────────────────────────────────────────────────
//
// The store is the source of truth for what's persisted; this `draft` ref is
// the editable form copy. Sync from `settingsStore.settings` on mount + on
// every `settings:updated` event. `null` until first `load()` resolves so
// the template can `v-if="draft"` to suppress the form during boot.
const draft = ref<HotkeyConfig | null>(null);

const isSaving = ref<boolean>(false);
const saveError = ref<string | null>(null);
const saveSuccess = ref<boolean>(false);
/** Pending auto-clear timer for the "✓ Saved" toast. Held so the cleanup in
 * `onUnmounted` can cancel it before the component tears down — otherwise a
 * stale `setTimeout` would fire on a disposed component. */
let successTimer: ReturnType<typeof setTimeout> | null = null;
/** How long the success indicator lingers before auto-clearing. Matches
 * SayIt's settings save UX. */
const SUCCESS_LINGER_MS = 2000;

/** All 6 preset trigger keys (mirrors the Rust `TriggerKey` enum's
 * non-Custom non-Combo variants — Phase 1 preset-only). Order matches the
 * SayIt UI for muscle-memory parity. */
const TRIGGER_KEY_OPTIONS: readonly TriggerKey[] = [
  "right-alt",
  "left-alt",
  "right-control",
  "left-control",
  "right-shift",
  "left-shift",
] as const;

// ─── Sync helpers ─────────────────────────────────────────────────────────

/** Reset the local draft from the store snapshot. Called on mount, after
 * save, and on each `settings:updated` event (so an out-of-band change in
 * another window — e.g. a future restore-defaults action — propagates). */
function syncDraftFromSettings(): void {
  if (settingsStore.settings) {
    // Spread to avoid wiring the form to the readonly proxy returned by the
    // store — we want a writable copy the user can edit before save.
    draft.value = { ...settingsStore.settings.hotkey };
  }
}

/** True when the local draft differs from the persisted store snapshot.
 * Drives the Save button's disabled state + the visibility of the Reset
 * button. */
const isDirty = computed<boolean>(() => {
  if (!settingsStore.settings || !draft.value) return false;
  const current = settingsStore.settings.hotkey;
  return (
    draft.value.triggerKey !== current.triggerKey ||
    draft.value.triggerMode !== current.triggerMode
  );
});

// ─── Event handlers ───────────────────────────────────────────────────────

async function handleSave(): Promise<void> {
  if (!draft.value || !isDirty.value) return;
  if (isSaving.value) return;
  isSaving.value = true;
  saveError.value = null;
  saveSuccess.value = false;
  // Cancel any prior pending auto-clear so a fast double-save doesn't leave
  // a stale timer running against the new "Saved" indicator.
  if (successTimer !== null) {
    clearTimeout(successTimer);
    successTimer = null;
  }
  try {
    // Spread again so the patch is a plain object, not the reactive ref —
    // matches the `update_settings` Rust signature `SettingsPatch { hotkey:
    // Option<HotkeyConfig> }` exactly.
    await settingsStore.update({ hotkey: { ...draft.value } });
    saveSuccess.value = true;
    successTimer = setTimeout(() => {
      saveSuccess.value = false;
      successTimer = null;
    }, SUCCESS_LINGER_MS);
  } catch (err) {
    saveError.value = err instanceof Error ? err.message : String(err);
  } finally {
    isSaving.value = false;
  }
}

/** Reset the draft back to the persisted state. Doesn't touch the store —
 * just discards local edits. */
function handleReset(): void {
  syncDraftFromSettings();
  saveError.value = null;
  saveSuccess.value = false;
}

// `<Select>` and `<RadioGroup>` from reka-ui emit `string | string[] | ...`
// via the `update:modelValue` channel. We narrow back to our enum types in
// these handlers so the `draft` ref stays strongly typed at the boundary.
function handleTriggerKeyChange(value: unknown): void {
  if (!draft.value) return;
  if (typeof value !== "string") return;
  draft.value = { ...draft.value, triggerKey: value as TriggerKey };
}

function handleTriggerModeChange(value: unknown): void {
  if (!draft.value) return;
  if (typeof value !== "string") return;
  draft.value = { ...draft.value, triggerMode: value as TriggerMode };
}

// ─── Lifecycle ────────────────────────────────────────────────────────────

onMounted(async () => {
  await settingsStore.load();
  await settingsStore.subscribe();
  syncDraftFromSettings();
});

onUnmounted(() => {
  if (successTimer !== null) {
    clearTimeout(successTimer);
    successTimer = null;
  }
  // Don't unsubscribe — the store's `settings:updated` listener is shared
  // across the Dashboard and is meant to live for the window's lifetime.
  // Component-local timers are the only thing we own here.
});

// Re-sync the draft whenever the store snapshot changes from outside (e.g.
// another window saved settings, hot-reload during dev, or a future
// restore-defaults flow). We only auto-pull when the user has no pending
// edits — overwriting in-flight edits would be hostile UX.
watch(
  () => settingsStore.settings,
  () => {
    if (!isDirty.value) {
      syncDraftFromSettings();
    }
  },
);
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="text-sm font-semibold">
        {{ t("views.settings.hotkey.title") }}
      </CardTitle>
    </CardHeader>
    <CardContent class="space-y-6">
      <!-- Trigger key (preset Select) -->
      <div class="space-y-2">
        <Label for="trigger-key">
          {{ t("views.settings.hotkey.triggerKey") }}
        </Label>
        <Select
          v-if="draft"
          :model-value="draft.triggerKey"
          @update:model-value="handleTriggerKeyChange"
        >
          <SelectTrigger
            id="trigger-key"
            class="w-72"
          >
            <SelectValue
              :placeholder="t('views.settings.hotkey.triggerKeyPlaceholder')"
            />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="opt in TRIGGER_KEY_OPTIONS"
              :key="opt"
              :value="opt"
            >
              {{ t(`views.settings.hotkey.triggerKeyOption.${opt}`) }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <!-- Trigger mode (RadioGroup) -->
      <div class="space-y-2">
        <Label>{{ t("views.settings.hotkey.triggerMode") }}</Label>
        <RadioGroup
          v-if="draft"
          :model-value="draft.triggerMode"
          class="space-y-2"
          @update:model-value="handleTriggerModeChange"
        >
          <div class="flex items-center space-x-2">
            <RadioGroupItem
              id="mode-hold"
              value="hold"
            />
            <Label for="mode-hold">
              {{ t("views.settings.hotkey.triggerModeHold") }}
            </Label>
          </div>
          <div class="flex items-center space-x-2">
            <RadioGroupItem
              id="mode-toggle"
              value="toggle"
            />
            <Label for="mode-toggle">
              {{ t("views.settings.hotkey.triggerModeToggle") }}
            </Label>
          </div>
        </RadioGroup>
      </div>

      <!-- Save / Reset row -->
      <div class="flex items-center gap-3">
        <Button
          :disabled="!isDirty || isSaving"
          @click="handleSave"
        >
          <Loader2
            v-if="isSaving"
            class="mr-2 size-4 animate-spin"
          />
          {{ t("views.settings.hotkey.save") }}
        </Button>
        <Button
          v-if="isDirty"
          variant="outline"
          :disabled="isSaving"
          @click="handleReset"
        >
          {{ t("views.settings.hotkey.reset") }}
        </Button>
        <span
          v-if="saveSuccess"
          class="text-xs text-primary"
          role="status"
          aria-live="polite"
        >
          {{ t("views.settings.hotkey.saveSuccess") }}
        </span>
        <span
          v-if="saveError"
          class="text-xs text-destructive"
          role="alert"
        >
          {{ saveError }}
        </span>
      </div>
    </CardContent>
  </Card>
</template>
