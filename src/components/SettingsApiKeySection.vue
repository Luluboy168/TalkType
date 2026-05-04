<script setup lang="ts">
// Settings → API key section (M3 chunks 1 + 3).
// Uses set_credential / has_credential / delete_credential. `get_credential`
// is Rust-only (architecture invariant #1) — the show/hide eye reveals only
// what the user typed THIS session, never a stored value. First-time save
// gates on <ProviderPrivacyDialog>. M3 active provider: `groq`; others are
// disabled with `(M6+)` suffix until M6 wires them up.

import { invoke } from "@tauri-apps/api/core";
import { Eye, EyeOff, ExternalLink, Loader2, Trash2 } from "lucide-vue-next";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import ProviderPrivacyDialog from "@/components/ProviderPrivacyDialog.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { LLM_PROVIDERS, findProvider } from "@/lib/providers";
import type { LlmProviderId, TestConnectionResult } from "@/types";

const { t } = useI18n();

// ─── State ────────────────────────────────────────────────────────────────

const selectedProvider = ref<LlmProviderId>("groq");
const apiKeyInput = ref<string>("");
const hasCredential = ref<boolean>(false);
/** Masked preview ("gsk_aBc…XyZ1") of the stored key so the user can
 * identify which key is currently saved. `null` when no key set or while
 * loading. Full key never crosses IPC — masking is Rust-side. */
const keyPreview = ref<string | null>(null);
const isSaving = ref<boolean>(false);
const isDeleting = ref<boolean>(false);
const isLoadingHasCredential = ref<boolean>(false);
const errorMessage = ref<string | null>(null);
const showPrivacyDialog = ref<boolean>(false);
/** Whether the password input shows characters (Eye icon) or hides them
 * (EyeOff icon). Affects ONLY the user's current-session typed input —
 * stored keys are never displayed. */
const isKeyVisible = ref<boolean>(false);

// ─── Test connection (M3 chunk 3, Q5) ─────────────────────────────────────

/** True while `invoke('test_provider_connection')` is in flight. */
const isTesting = ref<boolean>(false);
/** Human-readable result message after a test call (success or error).
 * `null` means "no result to show". Cleared automatically 5 s after a
 * successful test, or whenever the user switches provider / saves /
 * deletes a key (so the message doesn't stay stale next to a fresh key). */
const testResultMessage = ref<string | null>(null);
/** Discriminator for styling — success vs error vs in-progress. */
const testResultKind = ref<"success" | "error" | null>(null);
/** Pending auto-clear timer for `testResultMessage`. Held so we can
 * cancel + re-arm when a new test runs. */
let testResultClearTimer: ReturnType<typeof setTimeout> | null = null;
/** How long a successful test result lingers before auto-clearing. */
const TEST_RESULT_CLEAR_MS = 5_000;

const currentProvider = computed(() => findProvider(selectedProvider.value));
const isProviderActive = computed(() => currentProvider.value?.active ?? false);

const inputType = computed(() => (isKeyVisible.value ? "text" : "password"));

const placeholderText = computed(() => {
  const prefix = currentProvider.value?.expectedPrefix ?? "";
  if (!prefix) {
    return t("views.settings.apiKey.keyPlaceholder.generic");
  }
  return t("views.settings.apiKey.keyPlaceholder.withPrefix", { prefix });
});

// ─── Helpers ──────────────────────────────────────────────────────────────

async function refreshHasCredential(): Promise<void> {
  if (!isProviderActive.value) {
    hasCredential.value = false;
    keyPreview.value = null;
    return;
  }
  isLoadingHasCredential.value = true;
  errorMessage.value = null;
  try {
    // Use the masked preview command and infer hasCredential from its
    // result — saves a second IPC round-trip vs calling has_credential too.
    // Returns Some("gsk_aBc…XyZ1") when stored, None when no key set.
    const preview = await invoke<string | null>("get_credential_preview", {
      provider: selectedProvider.value,
    });
    keyPreview.value = preview;
    hasCredential.value = preview !== null;
  } catch (err) {
    errorMessage.value = formatError(err);
    hasCredential.value = false;
    keyPreview.value = null;
  } finally {
    isLoadingHasCredential.value = false;
  }
}

/** Maps Rust `CredentialsError` flat-string into a localized message.
 * Falls back to the raw string when no specific case matches.
 *
 * The Rust enum serializes as `"Unknown provider: foo"`, `"Empty API key"`,
 * etc. We do simple `startsWith` matching rather than parsing because the
 * frontend only needs to choose between a small set of localized branches.
 */
function formatError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  const provider = currentProvider.value?.displayName ?? selectedProvider.value;
  const prefix = currentProvider.value?.expectedPrefix ?? "";
  if (raw.startsWith("API key prefix does not match")) {
    return t("views.settings.apiKey.error.badPrefix", { provider, prefix });
  }
  if (raw === "Empty API key") {
    return t("views.settings.apiKey.error.empty");
  }
  if (raw.startsWith("API key is suspiciously long")) {
    return t("views.settings.apiKey.error.tooLong");
  }
  if (raw.startsWith("keyring error")) {
    // Truncate so the UI doesn't show a multi-line backend error verbatim.
    const detail = raw.replace(/^keyring error: /, "").slice(0, 80);
    return t("views.settings.apiKey.error.keyring", { detail });
  }
  return raw;
}

async function persistKey(): Promise<void> {
  if (!isProviderActive.value) return;
  if (apiKeyInput.value.trim().length === 0) {
    errorMessage.value = t("views.settings.apiKey.error.empty");
    return;
  }
  isSaving.value = true;
  errorMessage.value = null;
  // Saving a fresh key invalidates any previous test result.
  clearTestResult();
  try {
    await invoke<void>("set_credential", {
      provider: selectedProvider.value,
      key: apiKeyInput.value,
    });
    apiKeyInput.value = "";
    isKeyVisible.value = false;
    await refreshHasCredential();
  } catch (err) {
    errorMessage.value = formatError(err);
  } finally {
    isSaving.value = false;
  }
}

// ─── Event handlers ───────────────────────────────────────────────────────

async function handleSaveClick(): Promise<void> {
  // Idempotency guard: ignore reentrant clicks while a previous invoke is
  // still in flight (matches the audio-recorder section pattern).
  if (isSaving.value || isDeleting.value || isLoadingHasCredential.value) return;
  if (!isProviderActive.value) return;

  // First-time save → show privacy dialog. Re-saves (key rotation) skip
  // the disclosure since the user already consented previously.
  if (!hasCredential.value) {
    showPrivacyDialog.value = true;
    return;
  }
  await persistKey();
}

async function handlePrivacyConfirm(): Promise<void> {
  showPrivacyDialog.value = false;
  await persistKey();
}

function handlePrivacyCancel(): void {
  showPrivacyDialog.value = false;
  // Spec: cancel drops the typed key. We could keep it but the user will
  // re-confirm next click anyway — clearing avoids leaving sensitive
  // input lying around in the form.
  apiKeyInput.value = "";
}

async function handleDeleteClick(): Promise<void> {
  if (isSaving.value || isDeleting.value) return;
  if (!isProviderActive.value) return;
  isDeleting.value = true;
  errorMessage.value = null;
  // Stale test result no longer applies to a deleted key.
  clearTestResult();
  try {
    await invoke<void>("delete_credential", {
      provider: selectedProvider.value,
    });
    await refreshHasCredential();
  } catch (err) {
    errorMessage.value = formatError(err);
  } finally {
    isDeleting.value = false;
  }
}

/** Maps the Rust `TestConnectionError` flat-string into a localized message.
 *
 * The Rust `Display` impls (in `transcription/health.rs`) produce well-known
 * prefixes that we string-match here: `"Invalid API key (HTTP 401)"`,
 * `"Restricted API key (HTTP 403)"`, `"Rate limited ..."`, `"Network error: ..."`,
 * `"API key missing ..."`, `"Provider returned error 5xx: ..."`.
 *
 * Anything we don't recognise falls through to the generic `unknown` message
 * with the raw string included for debugging.
 */
function formatTestError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  if (raw.startsWith("Invalid API key")) {
    return t("views.settings.apiKey.testError.invalidKey");
  }
  if (raw.startsWith("Restricted API key")) {
    return t("views.settings.apiKey.testError.restrictedKey");
  }
  if (raw.startsWith("Rate limited")) {
    // Rust formats `RateLimited(Some(60))` as `"Rate limited (retry after Some(60)s)"`.
    // Pull the seconds out when present; otherwise show the no-seconds message.
    const match = raw.match(/Some\((\d+)\)/);
    if (match) {
      return t("views.settings.apiKey.testError.rateLimited", {
        seconds: match[1],
      });
    }
    return t("views.settings.apiKey.testError.rateLimitedNoSeconds");
  }
  if (raw.startsWith("Network error")) {
    return t("views.settings.apiKey.testError.network");
  }
  if (raw.startsWith("API key missing")) {
    return t("views.settings.apiKey.testError.missingKey");
  }
  return t("views.settings.apiKey.testError.unknown", { detail: raw });
}

/** Reset transient test-result UI state. Called on provider switch, save,
 * delete, before a fresh test, and on the auto-clear timer. */
function clearTestResult(): void {
  if (testResultClearTimer) {
    clearTimeout(testResultClearTimer);
    testResultClearTimer = null;
  }
  testResultMessage.value = null;
  testResultKind.value = null;
}

async function handleTestClick(): Promise<void> {
  if (isTesting.value || isSaving.value || isDeleting.value) return;
  if (!isProviderActive.value) return;
  if (!hasCredential.value) return;
  // Pre-flight: clear any previous result + cancel pending auto-clear so the
  // old message doesn't disappear mid-test from the prior schedule.
  clearTestResult();
  isTesting.value = true;
  try {
    const result = await invoke<TestConnectionResult>("test_provider_connection", {
      provider: selectedProvider.value,
    });
    testResultKind.value = "success";
    testResultMessage.value =
      result.modelCount !== null
        ? t("views.settings.apiKey.testSuccess", { count: result.modelCount })
        : t("views.settings.apiKey.testSuccessNoCount");
  } catch (err) {
    testResultKind.value = "error";
    testResultMessage.value = formatTestError(err);
  } finally {
    isTesting.value = false;
    // Auto-clear after 5 s — long enough to read, short enough not to
    // collide with the user retrying after a fix.
    testResultClearTimer = setTimeout(clearTestResult, TEST_RESULT_CLEAR_MS);
  }
}

function toggleKeyVisibility(): void {
  isKeyVisible.value = !isKeyVisible.value;
}

// ─── Lifecycle ────────────────────────────────────────────────────────────

watch(selectedProvider, () => {
  apiKeyInput.value = "";
  isKeyVisible.value = false;
  errorMessage.value = null;
  clearTestResult();
  void refreshHasCredential();
});

onMounted(() => {
  void refreshHasCredential();
});

onUnmounted(() => {
  // Cancel any pending auto-clear so the setTimeout callback doesn't fire
  // against a torn-down component.
  if (testResultClearTimer) {
    clearTimeout(testResultClearTimer);
    testResultClearTimer = null;
  }
});
</script>

<template>
  <section
    class="space-y-3 rounded-lg border border-border bg-card p-4 text-card-foreground"
    aria-labelledby="api-key-heading"
  >
    <header class="flex items-baseline justify-between gap-3">
      <h2
        id="api-key-heading"
        class="text-sm font-semibold"
      >
        {{ t("views.settings.apiKey.title") }}
      </h2>
      <a
        v-if="currentProvider"
        :href="currentProvider.consoleUrl"
        target="_blank"
        rel="noopener noreferrer"
        class="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground hover:underline"
      >
        {{ t("views.settings.apiKey.getKey") }}
        <ExternalLink class="size-3" />
      </a>
    </header>

    <div class="space-y-2">
      <label
        for="provider-select"
        class="text-xs font-medium text-muted-foreground"
      >
        {{ t("views.settings.apiKey.providerLabel") }}
      </label>
      <Select v-model="selectedProvider">
        <SelectTrigger
          id="provider-select"
          class="w-72"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="provider in LLM_PROVIDERS"
            :key="provider.id"
            :value="provider.id"
            :disabled="!provider.active"
          >
            {{ provider.displayName }}<span
              v-if="!provider.active"
              class="ml-1 text-muted-foreground"
            >({{ t("views.settings.apiKey.providerInactiveSuffix") }})</span>
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <div
      v-if="isProviderActive"
      class="space-y-2"
    >
      <label
        for="api-key-input"
        class="text-xs font-medium text-muted-foreground"
      >
        {{ t("views.settings.apiKey.keyLabel") }}
      </label>
      <div class="flex items-stretch gap-2">
        <div class="relative flex-1">
          <Input
            id="api-key-input"
            v-model="apiKeyInput"
            :type="inputType"
            :placeholder="placeholderText"
            autocomplete="off"
            spellcheck="false"
            class="pr-9"
            :disabled="isSaving || isDeleting"
          />
          <button
            type="button"
            :aria-label="
              isKeyVisible
                ? t('views.settings.apiKey.hide')
                : t('views.settings.apiKey.show')
            "
            class="absolute inset-y-0 right-0 flex w-8 items-center justify-center text-muted-foreground hover:text-foreground"
            @click="toggleKeyVisibility"
          >
            <Eye
              v-if="!isKeyVisible"
              class="size-4"
            />
            <EyeOff
              v-else
              class="size-4"
            />
          </button>
        </div>
        <Button
          size="sm"
          :disabled="
            isSaving ||
              isDeleting ||
              isLoadingHasCredential ||
              apiKeyInput.trim().length === 0
          "
          @click="handleSaveClick"
        >
          {{ t("views.settings.apiKey.save") }}
        </Button>
      </div>

      <div
        v-if="hasCredential"
        class="flex items-center justify-between gap-3 rounded-md bg-muted px-3 py-2"
      >
        <span class="flex items-center gap-2 text-xs font-medium text-foreground">
          <span>{{ t("views.settings.apiKey.saved") }}</span>
          <span
            v-if="keyPreview"
            class="font-mono text-muted-foreground"
            :title="t('views.settings.apiKey.previewTooltip')"
          >
            · {{ keyPreview }}
          </span>
        </span>
        <div class="flex items-center gap-1">
          <Button
            size="sm"
            variant="ghost"
            class="h-7 px-2"
            :disabled="isSaving || isDeleting || isTesting"
            @click="handleTestClick"
          >
            <Loader2
              v-if="isTesting"
              class="size-3.5 animate-spin"
            />
            <span class="ml-1">{{
              isTesting
                ? t("views.settings.apiKey.testing")
                : t("views.settings.apiKey.test")
            }}</span>
          </Button>
          <Button
            size="sm"
            variant="ghost"
            class="h-7 px-2 text-destructive hover:text-destructive"
            :disabled="isSaving || isDeleting || isTesting"
            @click="handleDeleteClick"
          >
            <Trash2 class="size-3.5" />
            <span class="ml-1">{{ t("views.settings.apiKey.delete") }}</span>
          </Button>
        </div>
      </div>

      <p
        v-if="testResultMessage"
        :class="[
          'text-xs',
          testResultKind === 'success' ? 'text-primary' : 'text-destructive',
        ]"
        role="status"
        aria-live="polite"
      >
        {{ testResultMessage }}
      </p>
    </div>

    <p
      v-else
      class="rounded-md bg-muted px-3 py-2 text-xs text-muted-foreground"
    >
      {{ t("views.settings.apiKey.providerInactiveHint") }}
    </p>

    <p
      v-if="errorMessage"
      class="text-xs text-destructive"
      role="alert"
    >
      {{ errorMessage }}
    </p>

    <ProviderPrivacyDialog
      v-model:open="showPrivacyDialog"
      :provider-id="selectedProvider"
      @confirm="handlePrivacyConfirm"
      @cancel="handlePrivacyCancel"
    />
  </section>
</template>

<style scoped>
/* Hide WebView2 / Edge / IE native password reveal button — we ship our own
   Eye / EyeOff toggle. Without this, Windows shows two eye icons stacked. */
:deep(input[type="password"])::-ms-reveal,
:deep(input[type="password"])::-ms-clear {
  display: none;
}
</style>
