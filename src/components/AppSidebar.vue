<script setup lang="ts">
// Dashboard sidebar — 5 navigation entries that mirror router.ts. Active
// highlighting is driven by the current vue-router route name.
import {
  BookOpen,
  HelpCircle,
  History,
  LayoutDashboard,
  type LucideIcon,
  Mic,
  Settings,
} from "lucide-vue-next";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import HudFlowBadge from "@/components/HudFlowBadge.vue";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";

interface NavItem {
  to: string;
  routeName: string;
  icon: LucideIcon;
  labelKey: string;
}

const { t } = useI18n();
const route = useRoute();

const items = computed<NavItem[]>(() => [
  { to: "/dashboard", routeName: "dashboard", icon: LayoutDashboard, labelKey: "sidebar.dashboard" },
  { to: "/history", routeName: "history", icon: History, labelKey: "sidebar.history" },
  { to: "/dictionary", routeName: "dictionary", icon: BookOpen, labelKey: "sidebar.dictionary" },
  { to: "/settings", routeName: "settings", icon: Settings, labelKey: "sidebar.settings" },
  { to: "/guide", routeName: "guide", icon: HelpCircle, labelKey: "sidebar.guide" },
]);

function isActive(item: NavItem): boolean {
  // Use matched chain so future child routes (e.g. /settings/hotkey) still
  // highlight the parent sidebar entry.
  return route.matched.some((r) => r.name === item.routeName);
}
</script>

<template>
  <Sidebar collapsible="icon">
    <SidebarHeader>
      <div class="flex items-center gap-2 px-2 py-1.5">
        <div
          class="flex size-8 shrink-0 items-center justify-center rounded-md bg-primary text-primary-foreground"
        >
          <Mic
            class="size-4"
            aria-hidden="true"
          />
        </div>
        <span class="text-sm font-semibold">{{ t("app.name") }}</span>
      </div>
    </SidebarHeader>
    <SidebarContent>
      <SidebarGroup>
        <SidebarGroupContent>
          <SidebarMenu>
            <SidebarMenuItem
              v-for="item in items"
              :key="item.routeName"
            >
              <SidebarMenuButton
                as-child
                :is-active="isActive(item)"
                :tooltip="t(item.labelKey)"
              >
                <RouterLink :to="item.to">
                  <component
                    :is="item.icon"
                    aria-hidden="true"
                  />
                  <span>{{ t(item.labelKey) }}</span>
                </RouterLink>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>
    </SidebarContent>
    <SidebarFooter>
      <HudFlowBadge />
    </SidebarFooter>
  </Sidebar>
</template>
