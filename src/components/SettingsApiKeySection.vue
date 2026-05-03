<script setup lang="ts">
// Settings → API key section (M3 chunk 1).
//
// Allows the user to store / delete API keys per provider in the OS
// credential vault via three Tauri commands:
//
//   * `has_credential(provider)`  — checks existence (no key value returned).
//   * `set_credential(provider, key)` — stores after validation.
//   * `delete_credential(provider)` — removes (idempotent).
//
// `get_credential` is intentionally absent — Rust-only per architecture
// invariant #1. The "show / hide key" toggle reveals only what the user
// typed in THIS session, never a value fetched back from keyring.
//
// **Privacy disclosure**: the first time a user saves a key for a given
// provider (i.e. `hasCredential === false` BEFORE save), a
// `<ProviderPrivacyDialog>` is shown explaining the audio-data flow. The
// user must explicitly confirm before the key is committed; cancel drops
// the input.
//
// **Phase 1 / M3** active provider: `groq`. The other three providers
// (`openai`, `anthropic`, `gemini`) are listed in the dropdown but
// disabled with `(M6+)` suffix — selecting them shows a placeholder
// message rather than allowing key entry.
//
// Extracted from `SettingsView.vue` to keep both files under the
// ~200-line component budget; M8 will further split `SettingsView` into
// per-area sub-components.

import { invoke } from "@tauri-apps/api/core";
import { Eye, EyeOff, ExternalLink, Trash2 } from "lucide-vue-next";
import { computed, onMounted, ref, watch } from "vue";
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
import type { LlmProviderId } from "@/types";

const { t } = useI18n();

// ─── State ────────────────────────────────────────────────────────────────

const selectedProvider = ref<LlmProviderId>("groq");
const apiKeyInput = ref<string>("");
const hasCredential = ref<boolean>(false);
const isSaving = ref<boolean>(false);
const isDeleting = ref<boolean>(false);
const isLoadingHasCredential = ref<boolean>(false);
const errorMessage = ref<string | null>(null);
const showPrivacyDialog = ref<boolean>(false);
/** Whether the password input shows characters (Eye icon) or hides them
 * (EyeOff icon). Affects ONLY the user's current-session typed input —
 * stored keys are never displayed. */
const isKeyVisible = ref<boolean>(false);

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
    return;
  }
  isLoadingHasCredential.value = true;
  errorMessage.value = null;
  try {
    hasCredential.value = await invoke<boolean>("has_credential", {
      provider: selectedProvider.value,
    });
  } catch (err) {
    errorMessage.value = formatError(err);
    hasCredential.value = false;
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

function toggleKeyVisibility(): void {
  isKeyVisible.value = !isKeyVisible.value;
}

// ─── Lifecycle ────────────────────────────────────────────────────────────

watch(selectedProvider, () => {
  apiKeyInput.value = "";
  isKeyVisible.value = false;
  errorMessage.value = null;
  void refreshHasCredential();
});

onMounted(() => {
  void refreshHasCredential();
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
        <span class="text-xs font-medium text-foreground">
          {{ t("views.settings.apiKey.saved") }}
        </span>
        <Button
          size="sm"
          variant="ghost"
          class="h-7 px-2 text-destructive hover:text-destructive"
          :disabled="isSaving || isDeleting"
          @click="handleDeleteClick"
        >
          <Trash2 class="size-3.5" />
          <span class="ml-1">{{ t("views.settings.apiKey.delete") }}</span>
        </Button>
      </div>
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
