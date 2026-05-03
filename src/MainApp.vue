<script setup lang="ts">
// Dashboard root view. Two-pane layout:
//   - left: AppSidebar (collapsible icon mode)
//   - right: SidebarInset wraps a thin header + RouterView
//
// SidebarProvider supplies the open/closed context that lets AppSidebar and
// SidebarTrigger talk to each other.
import { useI18n } from "vue-i18n";

import AppSidebar from "@/components/AppSidebar.vue";
import { Separator } from "@/components/ui/separator";
import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar";

const { t } = useI18n();

// Injected by Vite at build time (see vite.config.ts `define`).
const appVersion: string = __APP_VERSION__;
</script>

<template>
  <SidebarProvider>
    <AppSidebar />
    <SidebarInset>
      <header
        class="sticky top-0 z-10 flex h-12 shrink-0 items-center gap-2 border-b border-border bg-background px-4"
      >
        <SidebarTrigger class="-ml-1" />
        <Separator
          orientation="vertical"
          class="mx-2 h-4"
        />
        <div class="flex items-baseline gap-2">
          <span class="text-sm font-semibold">{{ t("app.name") }}</span>
          <span class="text-xs text-muted-foreground">
            {{ t("app.version_label") }} v{{ appVersion }}
          </span>
        </div>
      </header>
      <div class="flex-1 p-6">
        <RouterView />
      </div>
    </SidebarInset>
  </SidebarProvider>
</template>
