// Dashboard router. Hash mode keeps deep links working under the file://
// scheme that Tauri serves the bundled app from. Lazy imports make sure each
// view only loads when first navigated to.
import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";

const routes: RouteRecordRaw[] = [
  { path: "/", redirect: "/dashboard" },
  {
    path: "/dashboard",
    name: "dashboard",
    component: () => import("@/views/DashboardView.vue"),
  },
  {
    path: "/history",
    name: "history",
    component: () => import("@/views/HistoryView.vue"),
  },
  {
    path: "/dictionary",
    name: "dictionary",
    component: () => import("@/views/DictionaryView.vue"),
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("@/views/SettingsView.vue"),
  },
  {
    path: "/guide",
    name: "guide",
    component: () => import("@/views/FeatureGuideView.vue"),
  },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
