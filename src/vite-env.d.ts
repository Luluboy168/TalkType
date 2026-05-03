/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

// CSS-only side-effect imports from @fontsource-variable/* don't ship .d.ts.
declare module "@fontsource-variable/*";

declare const __APP_VERSION__: string;
