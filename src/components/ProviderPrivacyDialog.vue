<script setup lang="ts">
// Privacy disclosure dialog shown the FIRST time a user saves an API key
// for a given provider. Triggered from `SettingsView.vue` when the
// pre-save `has_credential` check is `false` (i.e. user is about to enter
// their API key for this provider for the first time).
//
// Per Settings UI spec (M3 chunk 1): "TalkType 會把你錄製的音訊傳送到 Groq
// (US) 進行轉錄。Groq 政策資料保留最多 14 天。" The exact body text is
// localized via `views.settings.apiKey.privacyDialog.body.<provider>`.
// Falls back to the generic `body.generic` for providers without a
// dedicated copy (M6 will add OpenAI / Anthropic / Gemini specifics).
//
// **Reuse**: this is a dumb component. Parent owns the open state, the
// selected provider, and the confirm/cancel handlers — keeping the dialog
// itself stateless makes it trivial to drop into M6's polish settings
// without modification.

import { computed } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { LlmProviderId } from "@/types";
import { findProvider } from "@/lib/providers";

interface Props {
  /** Two-way-bound open state. Parent flips to `true` to show the dialog. */
  open: boolean;
  /** Provider whose key is about to be saved — drives the body copy + title. */
  providerId: LlmProviderId;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  /** Reka-ui `update:open` for v-model:open binding. */
  (event: "update:open", value: boolean): void;
  /** Fired when the user clicks the confirm button. Parent should then
   * proceed with the actual `set_credential` invoke. */
  (event: "confirm"): void;
  /** Fired when the user cancels (clicks Cancel, ESC, overlay click, or
   * the close X). Parent should NOT save the key. */
  (event: "cancel"): void;
}>();

const { t, te } = useI18n();

const providerDisplayName = computed(
  () => findProvider(props.providerId)?.displayName ?? props.providerId,
);

/** Localized title — interpolates the provider's display name. */
const titleText = computed(() =>
  t("views.settings.apiKey.privacyDialog.title", { provider: providerDisplayName.value }),
);

/**
 * Localized body. We try a provider-specific key first
 * (`...body.groq`, `...body.openai`, ...) and fall back to a generic
 * message that names the provider. zh-TW + en both ship with `body.groq`;
 * M6 will add the other three with provider-specific privacy details.
 */
const bodyText = computed(() => {
  const specificKey = `views.settings.apiKey.privacyDialog.body.${props.providerId}`;
  const fallbackKey = "views.settings.apiKey.privacyDialog.body.generic";
  const key = te(specificKey) ? specificKey : fallbackKey;
  return t(key, { provider: providerDisplayName.value });
});

function handleOpenChange(next: boolean): void {
  if (!next) {
    // Dialog being dismissed (ESC, overlay click, or close X). Parent's
    // contract: "no save" — same as explicit Cancel.
    emit("cancel");
  }
  emit("update:open", next);
}

function handleConfirm(): void {
  emit("confirm");
  emit("update:open", false);
}

function handleCancel(): void {
  emit("cancel");
  emit("update:open", false);
}
</script>

<template>
  <Dialog
    :open="props.open"
    @update:open="handleOpenChange"
  >
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{{ titleText }}</DialogTitle>
        <DialogDescription>
          {{ bodyText }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button
          variant="outline"
          @click="handleCancel"
        >
          {{ t("views.settings.apiKey.privacyDialog.cancel") }}
        </Button>
        <Button @click="handleConfirm">
          {{ t("views.settings.apiKey.privacyDialog.confirm") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
