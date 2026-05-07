<script setup lang="ts">
// Settings → LLM Polish section (M6 chunk 4).
//
// One-stop UI for the polish path:
//   * polish toggle (Switch, tri-state Option<bool> per Decision #7)
//   * provider Select (4 active free-tier per Decision #3)
//   * model Select (filtered from `LLM_MODEL_LIST` by provider)
//   * preset RadioGroup (5 modes; custom shows the prompt textarea)
//   * custom prompt Textarea (visible only when preset === 'custom')
//   * retry toggle (Switch — Decision #5 retry-same once)
//   * Test polish button (F30 i18n-aware sample text, Decision #6 no retry)
//
// Top-of-section affordances rendered ALWAYS (independent of toggle state):
//   * F35 M5→M6 upgrade banner — dismissable, localStorage flag
//   * F29 per-step data-flow indicator — reactively updates on provider switch
//   * F22 no-key warning banner — only when polish is explicit ON + no key
//
// API-key invariant #1: this component MUST NOT call `get_credential` (only
// `has_credential`). The masked-preview helper `get_credential_preview` is
// not used here either — that's `SettingsApiKeySection`'s job.
//
// LOC budget per chunk-4 spec: ≤ 400. If we exceed that, we extract the
// data-flow indicator into its own dumb sub-component.

import { invoke } from "@tauri-apps/api/core";
import { Loader2, AlertTriangle, X } from "lucide-vue-next";
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import {
  LLM_PROVIDERS,
  LLM_MODEL_LIST,
  findProvider,
  getDefaultModelId,
  getModelsByProvider,
} from "@/lib/providers";
import { useSettingsStore } from "@/stores/useSettingsStore";
import {
  LLM_PROMPT_MODES,
  type LlmActivePolishProviderId,
  type LlmPromptMode,
  type PolishResult,
} from "@/types";

const { t } = useI18n();
const settingsStore = useSettingsStore();

// ─── Constants ────────────────────────────────────────────────────────────

/** Char limit shown in the custom-prompt char-count footer. Mirrors the
 * Rust `validate_custom_prompt` cap (chunk 1). */
const CUSTOM_PROMPT_MAX = 1000;

/** localStorage key for the F35 upgrade-banner dismissal flag. Persisted
 * per-browser-profile so reinstalling the app re-shows the banner once. */
const UPGRADE_NOTE_KEY = "talktype:m6_upgrade_seen";

/** Active providers — filtered from `LLM_PROVIDERS` so OpenAI / Anthropic
 * (still inactive in M6) don't appear in the dropdown. Per Decision #3
 * we ship 4 free-tier providers. */
const ACTIVE_PROVIDERS = computed(() =>
  LLM_PROVIDERS.filter((p) => p.active),
);

// ─── State ────────────────────────────────────────────────────────────────

/** Tri-state polish enable. `undefined` = auto-detect (None), `true` =
 * explicit ON, `false` = explicit OFF. Mirrors `Settings.llmPolishEnabled`
 * (Option<bool> in Rust, optional bool in TS). */
const polishEnabled = ref<boolean | undefined>(undefined);
const provider = ref<LlmActivePolishProviderId>("groq");
const modelId = ref<string>("llama-3.3-70b-versatile");
const promptMode = ref<LlmPromptMode>("default");
/**
 * Custom prompt working copy (the textarea binds here). Decoupled from
 * `customPromptSaved` so we can show explicit Save / Cancel buttons + a
 * dirty-state visual (grey when clean, black when edited). Replaces the
 * previous chunk-4 "blur-persist" behaviour after dogfood feedback that
 * users tabbing away with their mouse never blurred the textarea — and
 * the chunk-4 reviewer's P2 about a missing debounced fallback.
 */
const customPromptDraft = ref<string>("");
/**
 * Last persisted snapshot of `llmCustomPrompt`. The textarea text is
 * "clean" iff `customPromptDraft === customPromptSaved`. Driven by the
 * `settings.llmCustomPrompt` watch so a sibling-window edit propagates
 * here without clobbering an in-progress draft.
 */
const customPromptSaved = ref<string>("");
const retryEnabled = ref<boolean | undefined>(undefined);
/** Whether the chosen provider has a key in keyring. Drives the no-key
 * banner + the auto-detect tri-state visual. Refreshed on provider change
 * + after Settings updates (in case the user just saved a key in the
 * sibling API key section). */
const hasCredential = ref<boolean>(false);
/** Suppresses the upgrade banner permanently for this browser profile.
 * Read on mount; flipped + persisted on dismiss. */
const upgradeNoteSeen = ref<boolean>(true);
const isSaving = ref<boolean>(false);
const isTesting = ref<boolean>(false);
const testResult = ref<{ before: string; after: string } | null>(null);
const testError = ref<string | null>(null);

// ─── Derived ──────────────────────────────────────────────────────────────

/** Visual ON / OFF for the polish Switch:
 *   * `undefined` → reflects `hasCredential` (auto-detect)
 *   * `true` / `false` → explicit, ignore credential gate
 */
const polishVisualOn = computed<boolean>(() => {
  if (polishEnabled.value === true) return true;
  if (polishEnabled.value === false) return false;
  return hasCredential.value;
});

/** Provider's display name for the data-flow indicator + banner copy. */
const providerDisplayName = computed<string>(
  () => findProvider(provider.value)?.displayName ?? provider.value,
);

/** Models for the currently chosen provider (2 per active provider). */
const filteredModels = computed(() => getModelsByProvider(provider.value));

/** F22 no-key warning: only when polish is explicit ON and the chosen
 * provider has no credential. Auto-detect (None) silently skips polish —
 * no warning needed. */
const showNoKeyBanner = computed<boolean>(
  () => polishEnabled.value === true && !hasCredential.value,
);

/** Custom prompt char count (code-points, not bytes — CJK 1 char = 3
 * bytes is fine since the Rust cap also counts chars). */
const customPromptCharCount = computed<number>(() =>
  Array.from(customPromptDraft.value).length,
);

const customPromptTooLong = computed<boolean>(
  () => customPromptCharCount.value > CUSTOM_PROMPT_MAX,
);

/** Dirty when the working draft differs from the persisted snapshot.
 * Drives both the textarea text colour (muted-foreground when clean,
 * foreground when dirty) and the save / cancel buttons' disabled state. */
const customPromptDirty = computed<boolean>(
  () => customPromptDraft.value !== customPromptSaved.value,
);

/** Save button disabled when not dirty OR over the 1000-char cap. The
 * cap check uses raw `.length` rather than `customPromptCharCount` so
 * the boundary case (exactly 1000 chars) stays enabled — matches the
 * Rust `validate_custom_prompt` behaviour. */
const customPromptSaveDisabled = computed<boolean>(
  () =>
    !customPromptDirty.value || customPromptDraft.value.length > CUSTOM_PROMPT_MAX,
);
/** Cancel button disabled when not dirty (no work to revert). */
const customPromptCancelDisabled = computed<boolean>(
  () => !customPromptDirty.value,
);

const customPromptCountLabel = computed<string>(() =>
  t("views.settings.llmPolish.customPrompt.charCount", {
    n: customPromptCharCount.value,
  }),
);

/** F31: Test polish button is the ONLY control disabled by polish toggle
 * OFF. Provider / model / preset / custom remain enabled for pre-config. */
const testButtonDisabled = computed<boolean>(
  () =>
    polishEnabled.value === false ||
    isTesting.value ||
    isSaving.value ||
    customPromptTooLong.value,
);

const testDisabledTooltip = computed<string | undefined>(() =>
  polishEnabled.value === false
    ? t("views.settings.llmPolish.testDisabledTooltip")
    : undefined,
);

// ─── Sync helpers ─────────────────────────────────────────────────────────

/** Pull the latest settings snapshot into the local refs. Called on mount,
 * on every `settings:updated` (via the store subscription), and after
 * provider switch (to refresh the credential gate). */
function syncFromStore(): void {
  const s = settingsStore.settings;
  if (!s) return;
  polishEnabled.value = s.llmPolishEnabled;
  // Default to groq when the store has nothing or the persisted provider
  // is one of the inactive entries (openai / anthropic) — defensive
  // because v0.2 may activate them later.
  const persistedProvider = s.llmProvider;
  if (
    persistedProvider === "groq" ||
    persistedProvider === "openrouter" ||
    persistedProvider === "nvidia" ||
    persistedProvider === "gemini"
  ) {
    provider.value = persistedProvider;
  } else {
    provider.value = "groq";
  }
  // Resolve effective model id: stored value if present, else provider default.
  const persistedModel = s.llmModelId;
  if (
    persistedModel &&
    LLM_MODEL_LIST.some(
      (m) => m.id === persistedModel && m.provider === provider.value,
    )
  ) {
    modelId.value = persistedModel;
  } else {
    modelId.value = getDefaultModelId(provider.value);
  }
  promptMode.value = s.llmPromptMode ?? "default";
  // Sync the persisted snapshot. We capture "was the textarea dirty
  // BEFORE this sync?" first — if the user has an in-progress edit (draft
  // !== old saved), don't clobber their draft when an external Settings
  // write fans in. Order matters: read `customPromptDirty` before mutating
  // `customPromptSaved`, otherwise the dirty check would always see a
  // freshly-synced saved value and false-negative.
  const persistedCustomPrompt = s.llmCustomPrompt ?? "";
  const wasDirty = customPromptDirty.value;
  customPromptSaved.value = persistedCustomPrompt;
  if (!wasDirty) {
    customPromptDraft.value = persistedCustomPrompt;
  }
  retryEnabled.value = s.llmPolishRetryEnabled;
}

/** Refresh `hasCredential` for the currently chosen provider. Called on
 * mount + on provider switch. Errors are swallowed silently — the no-key
 * banner already covers the no-key case, and a `has_credential` failure
 * doesn't justify blocking the section. */
async function refreshHasCredential(): Promise<void> {
  try {
    hasCredential.value = await invoke<boolean>("has_credential", {
      provider: provider.value,
    });
  } catch (err) {
    console.warn("[settings-llm-polish] has_credential failed:", err);
    hasCredential.value = false;
  }
}

// ─── Event handlers ───────────────────────────────────────────────────────

/** Persist a single field via SettingsPatch. We do per-field saves so
 * the user gets immediate feedback on each change (matches the
 * SettingsHotkeySection pattern, but without an explicit Save button). */
async function patchSettings(
  patch: Parameters<typeof settingsStore.update>[0],
): Promise<void> {
  if (isSaving.value) return;
  isSaving.value = true;
  try {
    await settingsStore.update(patch);
  } catch (err) {
    console.error("[settings-llm-polish] update_settings failed:", err);
  } finally {
    isSaving.value = false;
  }
}

function handleTogglePolish(value: boolean): void {
  // Switch flips between explicit ON and explicit OFF — there's no UI
  // path back to None auto-detect in M6 (Reset link defers to v0.2).
  polishEnabled.value = value;
  void patchSettings({ llmPolishEnabled: value });
}

function handleToggleRetry(value: boolean): void {
  retryEnabled.value = value;
  void patchSettings({ llmPolishRetryEnabled: value });
}

function handleProviderChange(value: unknown): void {
  if (typeof value !== "string") return;
  if (
    value !== "groq" &&
    value !== "openrouter" &&
    value !== "nvidia" &&
    value !== "gemini"
  ) {
    return;
  }
  provider.value = value;
  // Auto-pick the provider's default model so the model dropdown isn't
  // showing a now-orphaned id (e.g. switched groq → gemini and the prior
  // model was Llama).
  const newDefault = getDefaultModelId(value);
  modelId.value = newDefault;
  void patchSettings({
    llmProvider: value,
    llmModelId: newDefault,
  });
  void refreshHasCredential();
  // Stale test result no longer applies to a different provider.
  testResult.value = null;
  testError.value = null;
}

function handleModelChange(value: unknown): void {
  if (typeof value !== "string") return;
  modelId.value = value;
  void patchSettings({ llmModelId: value });
}

function handlePresetChange(value: unknown): void {
  if (typeof value !== "string") return;
  if (!LLM_PROMPT_MODES.includes(value as LlmPromptMode)) return;
  promptMode.value = value as LlmPromptMode;
  void patchSettings({ llmPromptMode: value as LlmPromptMode });
}

function handleCustomPromptInput(value: string): void {
  // Mutate only the draft. Persistence happens on explicit Save click —
  // dogfood replaced the chunk-4 blur-persist with explicit save / cancel
  // buttons + a dirty-state visual.
  customPromptDraft.value = value;
}

async function handleCustomPromptSave(): Promise<void> {
  if (customPromptSaveDisabled.value) return;
  await patchSettings({ llmCustomPrompt: customPromptDraft.value });
  // Optimistic local clean-state. The Rust `settings:updated` event will
  // also fire and walk through `syncFromStore`, but flipping the saved
  // snapshot here gives the user immediate feedback (textarea returns to
  // muted grey on click rather than waiting for the round-trip).
  customPromptSaved.value = customPromptDraft.value;
}

function handleCustomPromptCancel(): void {
  if (customPromptCancelDisabled.value) return;
  customPromptDraft.value = customPromptSaved.value;
}

async function handleTestPolish(): Promise<void> {
  if (testButtonDisabled.value) return;
  isTesting.value = true;
  testResult.value = null;
  testError.value = null;
  const sample = t("views.settings.llmPolish.testSample");
  try {
    const result = await invoke<PolishResult>("polish_text", {
      args: {
        rawText: sample,
        vocabulary: [],
        attempt: 1,
      },
    });
    testResult.value = { before: sample, after: result.polishedText };
  } catch (err) {
    const detail = err instanceof Error ? err.message : String(err);
    testError.value = t("views.settings.llmPolish.testFailed", { detail });
  } finally {
    isTesting.value = false;
  }
}

function dismissUpgradeNote(): void {
  upgradeNoteSeen.value = true;
  try {
    window.localStorage.setItem(UPGRADE_NOTE_KEY, "1");
  } catch (err) {
    // localStorage might be unavailable in some test envs — non-fatal.
    console.warn("[settings-llm-polish] localStorage write failed:", err);
  }
}

// ─── Lifecycle ────────────────────────────────────────────────────────────

watch(
  () => settingsStore.settings,
  () => {
    syncFromStore();
  },
);

onMounted(async () => {
  await settingsStore.load();
  await settingsStore.subscribe();
  syncFromStore();
  await refreshHasCredential();
  try {
    upgradeNoteSeen.value =
      window.localStorage.getItem(UPGRADE_NOTE_KEY) === "1";
  } catch {
    // SSR / restricted contexts: assume seen = true (don't pop banner).
    upgradeNoteSeen.value = true;
  }
});
</script>

<template>
  <section
    class="space-y-3 rounded-lg border border-border bg-card p-4 text-card-foreground"
    aria-labelledby="llm-polish-heading"
  >
    <header class="flex items-baseline justify-between gap-3">
      <h2
        id="llm-polish-heading"
        class="text-sm font-semibold"
      >
        {{ t("views.settings.llmPolish.title") }}
      </h2>
    </header>

    <!-- F35 M5 → M6 upgrade banner (dismissable, localStorage flag) -->
    <div
      v-if="!upgradeNoteSeen"
      role="region"
      aria-labelledby="m6-upgrade-banner-title"
      class="flex items-start gap-3 rounded-md border border-amber-500/40 bg-amber-50 p-3 text-amber-900 dark:bg-amber-900/20 dark:text-amber-200"
      data-testid="m6-upgrade-banner"
    >
      <AlertTriangle class="mt-0.5 size-4 shrink-0" />
      <div class="flex-1 space-y-1 text-xs">
        <p
          id="m6-upgrade-banner-title"
          class="font-semibold"
        >
          {{ t("views.settings.upgradeNote.m5ToM6.title") }}
        </p>
        <p class="leading-relaxed">
          {{ t("views.settings.upgradeNote.m5ToM6.body") }}
        </p>
      </div>
      <button
        type="button"
        :aria-label="t('views.settings.upgradeNote.m5ToM6.dismiss')"
        class="rounded text-amber-900 hover:bg-amber-200/40 dark:text-amber-200"
        data-testid="m6-upgrade-banner-dismiss"
        @click="dismissUpgradeNote"
      >
        <X class="size-4" />
      </button>
    </div>

    <!-- F29 per-step data-flow indicator (top, always visible) -->
    <div
      class="flex flex-wrap items-center gap-2 rounded-md bg-muted px-3 py-2 text-xs text-muted-foreground"
      data-testid="dataflow-indicator"
      role="group"
      :aria-label="t('views.settings.llmPolish.title')"
    >
      <span>{{ t("views.settings.llmPolish.dataFlow.audio") }}</span>
      <span aria-hidden="true">→</span>
      <span>
        {{
          t("views.settings.llmPolish.dataFlow.whisper", { provider: "Groq" })
        }}
      </span>
      <span aria-hidden="true">→</span>
      <template v-if="polishVisualOn">
        <span :class="{ 'opacity-60': !hasCredential }">
          {{
            t("views.settings.llmPolish.dataFlow.polish", {
              provider: providerDisplayName,
            })
          }}
          <span v-if="!hasCredential">
            {{ t("views.settings.llmPolish.disabled") }}
          </span>
        </span>
        <span aria-hidden="true">→</span>
      </template>
      <span>{{ t("views.settings.llmPolish.dataFlow.paste") }}</span>
      <span class="text-muted-foreground/70">
        ({{ t("views.settings.llmPolish.dataFlow.noRetention") }})
      </span>
      <span
        v-if="!polishVisualOn"
        class="text-muted-foreground/70"
      >
        {{ t("views.settings.llmPolish.dataFlow.withoutPolish") }}
      </span>
    </div>

    <!-- F22 no-key warning banner (polish ON + no key) -->
    <p
      v-if="showNoKeyBanner"
      role="alert"
      class="rounded-md border border-amber-500/40 bg-amber-50 px-3 py-2 text-xs text-amber-900 dark:bg-amber-900/20 dark:text-amber-200"
      data-testid="no-key-banner"
    >
      {{
        t("views.settings.llmPolish.banner.noKey", {
          provider: providerDisplayName,
        })
      }}
    </p>

    <!-- Polish toggle -->
    <div class="flex items-start justify-between gap-3">
      <div class="flex-1 space-y-1">
        <Label
          for="llm-polish-enable"
          class="text-xs font-medium"
        >
          {{ t("views.settings.llmPolish.enable") }}
        </Label>
        <p class="text-xs text-muted-foreground">
          {{ t("views.settings.llmPolish.enableDescription") }}
        </p>
      </div>
      <Switch
        id="llm-polish-enable"
        :model-value="polishVisualOn"
        data-testid="polish-toggle"
        @update:model-value="
          (next: boolean) => handleTogglePolish(next === true)
        "
      />
    </div>

    <!-- Provider Select -->
    <div class="space-y-2">
      <Label
        for="llm-provider-select"
        class="text-xs font-medium text-muted-foreground"
      >
        {{ t("views.settings.llmPolish.providerLabel") }}
      </Label>
      <Select
        :model-value="provider"
        @update:model-value="handleProviderChange"
      >
        <SelectTrigger
          id="llm-provider-select"
          class="w-72"
          data-testid="provider-select"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="p in ACTIVE_PROVIDERS"
            :key="p.id"
            :value="p.id"
          >
            {{ p.displayName }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <!-- Model Select (filtered) -->
    <div class="space-y-2">
      <Label
        for="llm-model-select"
        class="text-xs font-medium text-muted-foreground"
      >
        {{ t("views.settings.llmPolish.modelLabel") }}
      </Label>
      <Select
        :model-value="modelId"
        @update:model-value="handleModelChange"
      >
        <SelectTrigger
          id="llm-model-select"
          class="w-72"
          data-testid="model-select"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="m in filteredModels"
            :key="m.id"
            :value="m.id"
          >
            {{ m.displayName }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <!-- Preset RadioGroup -->
    <div class="space-y-2">
      <Label class="text-xs font-medium text-muted-foreground">
        {{ t("views.settings.llmPolish.presetLabel") }}
      </Label>
      <RadioGroup
        :model-value="promptMode"
        class="grid grid-cols-1 gap-1.5 sm:grid-cols-2"
        data-testid="preset-radio-group"
        @update:model-value="handlePresetChange"
      >
        <div
          v-for="mode in LLM_PROMPT_MODES"
          :key="mode"
          class="flex items-start gap-2 rounded-md border border-border bg-background p-2"
        >
          <RadioGroupItem
            :id="`llm-preset-${mode}`"
            :value="mode"
            class="mt-0.5"
          />
          <div class="flex-1 space-y-0.5">
            <Label
              :for="`llm-preset-${mode}`"
              class="text-xs font-medium"
            >
              {{ t(`views.settings.llmPolish.preset.${mode}.label`) }}
            </Label>
            <p class="text-xs text-muted-foreground">
              {{ t(`views.settings.llmPolish.preset.${mode}.description`) }}
            </p>
          </div>
        </div>
      </RadioGroup>
    </div>

    <!-- Custom prompt textarea (only when preset === custom) -->
    <div
      v-if="promptMode === 'custom'"
      class="space-y-1.5"
      data-testid="custom-prompt-section"
    >
      <Label
        for="llm-custom-prompt"
        class="text-xs font-medium text-muted-foreground"
      >
        {{ t("views.settings.llmPolish.customPromptLabel") }}
      </Label>
      <Textarea
        id="llm-custom-prompt"
        :model-value="customPromptDraft"
        :placeholder="t('views.settings.llmPolish.customPrompt.placeholder')"
        rows="4"
        data-testid="custom-prompt-textarea"
        :class="
          customPromptDirty ? 'text-foreground' : 'text-muted-foreground'
        "
        @update:model-value="handleCustomPromptInput"
      />
      <div class="flex items-center justify-between text-xs">
        <span
          v-if="customPromptTooLong"
          class="text-destructive"
        >
          {{ t("views.settings.llmPolish.customPrompt.tooLong") }}
        </span>
        <span v-else />
        <span
          :class="
            customPromptTooLong ? 'text-destructive' : 'text-muted-foreground'
          "
          data-testid="custom-prompt-count"
        >
          {{ customPromptCountLabel }}
        </span>
      </div>
      <!-- Save / Cancel buttons (dogfood: explicit persist + revert) -->
      <div class="flex justify-end gap-2">
        <Button
          size="sm"
          variant="outline"
          :disabled="customPromptCancelDisabled"
          data-testid="custom-prompt-cancel"
          @click="handleCustomPromptCancel"
        >
          {{ t("views.settings.llmPolish.customPrompt.cancel") }}
        </Button>
        <Button
          size="sm"
          :disabled="customPromptSaveDisabled"
          data-testid="custom-prompt-save"
          @click="handleCustomPromptSave"
        >
          {{ t("views.settings.llmPolish.customPrompt.save") }}
        </Button>
      </div>
    </div>

    <!-- Retry toggle (Decision #5) -->
    <div class="flex items-start justify-between gap-3">
      <div class="flex-1 space-y-1">
        <Label
          for="llm-polish-retry"
          class="text-xs font-medium"
        >
          {{ t("views.settings.llmPolish.retry.label") }}
        </Label>
        <p class="text-xs text-muted-foreground">
          {{ t("views.settings.llmPolish.retry.description") }}
        </p>
      </div>
      <Switch
        id="llm-polish-retry"
        :model-value="retryEnabled !== false"
        data-testid="retry-toggle"
        @update:model-value="
          (next: boolean) => handleToggleRetry(next === true)
        "
      />
    </div>

    <!-- Test polish button (F30 i18n-aware sample, F31 only-this-disabled) -->
    <div class="space-y-2">
      <div class="flex items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          :disabled="testButtonDisabled"
          :title="testDisabledTooltip"
          data-testid="test-polish-button"
          @click="handleTestPolish"
        >
          <Loader2
            v-if="isTesting"
            class="size-3.5 animate-spin"
          />
          <span class="ml-1">
            {{
              isTesting
                ? t("views.settings.llmPolish.testing")
                : t("views.settings.llmPolish.testButton")
            }}
          </span>
        </Button>
      </div>
      <div
        v-if="testResult"
        class="space-y-1 rounded-md bg-muted px-3 py-2 text-xs"
        data-testid="test-result"
      >
        <p>
          <span class="font-semibold">
            {{ t("views.settings.llmPolish.testBefore") }}:
          </span>
          {{ testResult.before }}
        </p>
        <p>
          <span class="font-semibold">
            {{ t("views.settings.llmPolish.testAfter") }}:
          </span>
          {{ testResult.after }}
        </p>
      </div>
      <p
        v-if="testError"
        class="text-xs text-destructive"
        role="alert"
        data-testid="test-error"
      >
        {{ testError }}
      </p>
    </div>
  </section>
</template>
