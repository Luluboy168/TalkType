# 技術選型與理由

> **狀態**：Draft v1
> **最後更新**：2026-05-02

## TL;DR

**Tauri v2 (Rust) + Vue 3 + TypeScript**，與 SayIt 一致。理由：cross-platform from day 1、SayIt 已驗證可達 < 3 秒延遲、Rust 音頻 pipeline 穩、binary 小、學習曲線陡但有完整 reference (SayIt CLAUDE.md + 我們的 doc/reference/)。

## 主要技術棧

| 層 | 選擇 | 版本 | 理由 |
|---|---|---|---|
| **Desktop framework** | Tauri | v2 | Native + Web hybrid、binary 小、Rust 後端強 |
| **後端語言** | Rust | stable | 已驗證 by SayIt；audio (cpal/hound/rustfft) 跟 cross-platform input (rdev/windows) 生態完整 |
| **前端框架** | Vue 3 | ^3.5 | Composition API + `<script setup>`、shadcn-vue 生態 |
| **語言** | TypeScript | ^5.7 | 嚴格 type safety、IPC payload 驗證 |
| **State management** | Pinia | ^3.0 | Vue 3 官方推薦、composition style |
| **Routing** | Vue Router | ^5（hash mode）| Tauri file:// protocol 相容 |
| **CSS** | Tailwind v4 | ^4 | 配合 shadcn-vue、語意 token system |
| **UI components** | shadcn-vue | new-york style | Vue port of shadcn/ui、不是 npm package、複製進 src/components/ui/ |
| **UI primitives** | reka-ui | ^2 | Vue port of Radix UI、shadcn-vue 底層 |
| **Icons** | lucide-vue-next | latest | Tree-shakeable、設計一致 |
| **i18n** | vue-i18n | ^11 | 業界標準 |
| **Build tool** | Vite | ^6 | Tauri 預設、HMR 快 |
| **Package manager** | pnpm | 10.x（pin）| 比 npm 快、disk 省、workspace 支援 |
| **Node** | 24 (LTS) | pin via `.nvmrc` | 跟 SayIt 一致 |

## Rust crates

### 核心

| Crate | 版本 | 用途 |
|---|---|---|
| `tauri` | 2.x | Desktop framework |
| `tauri-build` | 2.x | Build-time codegen |
| `tauri-plugin-shell` | 2.x | Open URLs |
| `tauri-plugin-http` | 2.x | Bypass browser CSP for LLM API calls |
| `tauri-plugin-sql` (sqlite) | 2.3.x | SQLite |
| `tauri-plugin-store` | 2.4.x | JSON KV store（settings） |
| `tauri-plugin-autostart` | 2.5.x | Boot-on-login |
| `tauri-plugin-process` | 2.x | Restart |
| `tauri-plugin-single-instance` | 2.x | 防止多 instance |
| `serde` (derive) + `serde_json` | 1.x | IPC serialization |
| `thiserror` | 2.x | Typed errors |

### Audio

| Crate | 版本 | 用途 |
|---|---|---|
| `cpal` | 0.15.x | Cross-platform audio capture |
| `hound` | 3.5.x | WAV encoding |
| `rustfft` | 6.x | FFT for waveform visualization |

### Networking

| Crate | 版本 | 用途 |
|---|---|---|
| `reqwest` (multipart, json) | 0.12.x | HTTP client（給 Rust-side Whisper API call） |

### Clipboard

| Crate | 版本 | 用途 |
|---|---|---|
| `arboard` | 3.x | Cross-platform clipboard |

### **TalkType 對 SayIt 的關鍵改進**

| Crate | 版本 | 用途 | 為什麼加 |
|---|---|---|---|
| `keyring` | 3.x | OS Credential Vault 整合（Windows Credential Manager / macOS Keychain） | **取代 SayIt 的 plaintext settings.json 存 API key** — 重大安全改進 |
| `whisper-rs` 或 `whisper-cpp-2` | latest | whisper.cpp Rust binding | **本地 transcription** — SayIt 沒有 |

> **whisper.cpp binding 候選**：
> - `whisper-rs` — 較成熟、API 穩、依賴 whisper.cpp 系統 lib 或 vendored build
> - `whisper-cpp-2` — 較新、可能更靈活
> - 直接 FFI to whisper.cpp — 最大控制但工作量最大
>
> M7 milestone（local transcription 整合）會做 spike 決定。

### Windows-only

| Crate | 版本 | 用途 |
|---|---|---|
| `windows` (selected features) | 0.61.x | Win32 API：Hooks、SendInput、WASAPI、PlaySoundA |

### macOS-only（Phase 2）

| Crate | 版本 | 用途 |
|---|---|---|
| `core-graphics` | 0.24.x | CGEvent |
| `core-foundation` | 0.10.x | CFRunLoop / CFString |
| `objc` | 0.2.x | NSWindow setLevel: |

### Telemetry（Phase 2）

| Crate | 版本 | 用途 |
|---|---|---|
| `sentry` | 0.46.x | Error / perf monitoring |

## 前端 dependencies

### Runtime

| Package | 版本 | 用途 |
|---|---|---|
| `@tauri-apps/api` | ^2 | invoke / event / window |
| `@tauri-apps/plugin-http` | ^2 | LLM API calls（必須用、不能用 window.fetch）|
| `@tauri-apps/plugin-sql` | ^2 | SQLite frontend client（**Phase 1 評估**：可能改成 Rust 擁有 SQL，frontend 走 commands — 對 SayIt 改進）|
| `@tauri-apps/plugin-store` | ^2 | Settings JSON |
| `@tauri-apps/plugin-shell` | ^2 | Open URLs |
| `@tauri-apps/plugin-process` | ^2 | Restart |
| `@tauri-apps/plugin-autostart` | ^2 | Boot-on-login |
| `@tauri-apps/plugin-updater` | ^2 | Phase 2 only |
| `vue` | ^3.5 | Framework |
| `vue-router` | ^5（hash） | Routing |
| `pinia` | ^3 | State management |
| `vue-i18n` | ^11 | i18n |
| `reka-ui` | ^2 | UI primitives |
| `lucide-vue-next` | latest | Icons |
| `class-variance-authority` | ^0.7 | shadcn variant API |
| `clsx` | ^2 | className helper |
| `tailwind-merge` | ^3 | className merge |
| `@vueuse/core` | ^14 | useRafFn 等 utilities |

### Phase 2 加

| Package | 版本 | 用途 |
|---|---|---|
| `@sentry/vue` | ^10 | Telemetry |
| `@unovis/vue` + `@unovis/ts` | ^1.6 | Dashboard 統計圖表 |
| `@tanstack/vue-table` | ^8 | History 表格 |

### Dev

| Package | 版本 | 用途 |
|---|---|---|
| `vite` | ^6 | Build |
| `@vitejs/plugin-vue` | ^5 | Vue support |
| `@tailwindcss/vite` + `tailwindcss` + `tw-animate-css` | ^4 | Styling |
| `@tauri-apps/cli` | ^2 | Tauri CLI |
| `typescript` + `vue-tsc` | latest | Type checking |
| `eslint` + `eslint-plugin-vue` + `typescript-eslint` + `@eslint/js` | latest | Linting |
| `vitest` + `@vitest/coverage-v8` + `@vue/test-utils` + `jsdom` | latest | Unit + component tests |
| `@playwright/test` | ^1 | E2E tests（Phase 2 主要）|
| `@faker-js/faker` | latest | Test data factories |

## 為什麼不選其他方案

### 為什麼不選 Electron

- **+** 巨大社群、極熟悉的 web 技術、Chromium 一致性
- **−** 150MB+ binary、120MB+ idle 記憶體、Node.js runtime 重
- **−** 全域熱鍵與 paste 需要 native modules（uIOhook 等），複雜度跟 Tauri 差不多
- **判決**：對音頻 app 來說 binary size 不是 sell point，但 idle memory 是。Electron 在 always-running daemon 場景太重。

### 為什麼不選 .NET 8 + WinUI 3 / WPF

- **+** Windows 一等公民、Win32 API 直接、Visual Studio 工具鏈
- **+** 可能比 Tauri stable
- **−** **未來跨平台要重寫** — 違反 Phase 2 macOS goal
- **−** XAML 不熟、社群比 Vue 小
- **判決**：違反 cross-platform target、否決

### 為什麼不選 Wails v3 (Go)

- **+** Go 生態、binary 小、cross-platform
- **−** Go GUI 生態不如 Rust 完整（cpal、whisper-rs 都是 Rust）
- **−** Wails v3 還在 alpha
- **判決**：cpal + whisper-rs 是 Rust，跨語言 FFI 折磨

### 為什麼不選 Flutter Desktop

- **+** Cross-platform、UI framework 成熟
- **−** Dart 不熟、桌面定位 less mature than mobile
- **−** Flutter 跟原生 OS API 整合 painful（hotkey、paste injection）
- **判決**：太遠離 Web 技術、不選

### 為什麼不選 Python + PyQt / PySide

- **+** Whisper / 各家 AI lib Python 最豐富
- **+** 開發快
- **−** 打包成單檔 .exe painful（PyInstaller + 解壓 startup 慢）
- **−** Python GIL 對 audio processing 不友善
- **−** 部署 Python runtime 跟使用者
- **判決**：分發成本太高，否決

### 為什麼不選 Rust 純 native（egui / Slint / Iced）

- **+** 一個語言搞定、binary 最小
- **−** Rust GUI 生態不成熟
- **−** 設計師不熟 Rust UI 框架
- **−** shadcn-vue 等成熟 UI lib 用不上
- **判決**：UI 開發效率太低

## 版本管理策略

### Lock files

- **`pnpm-lock.yaml`** — committed、pre-commit hook 防止手動修改（學 SayIt）
- **`Cargo.lock`** — committed（binary crate）、pre-commit hook 防止手動修改

### 版本 pin

- Node `.nvmrc` 寫死 `24`
- pnpm `package.json` `packageManager` 寫死 `pnpm@10.x.x`
- Rust 用 `rust-toolchain.toml` pin 到 stable channel（不寫死 1.x.x，跟 stable）

### 主要 dependency 升級節奏

- Tauri v2 — 跟 minor version 升（穩定後）
- Vue / Pinia — minor 升、major 評估
- shadcn-vue components — 手動 sync（複製進 repo）
- Rust crates — quarterly review
- Lock 升級不 deferred、定期 `pnpm update --interactive`

## CSP（Content Security Policy）

`tauri.conf.json` 預計：

```json
"csp": "default-src 'self'; connect-src 'self' https://api.groq.com https://api.openai.com https://api.anthropic.com https://generativelanguage.googleapis.com; style-src 'self' 'unsafe-inline'; script-src 'self'; media-src 'self' blob:"
```

對 SayIt 的改進：

- `connect-src` 列**全部**4 個 LLM provider（SayIt 漏了 OpenAI/Anthropic）
- `media-src` 不包含 `http://asset.localhost`（因為我們會走 Rust IPC + Blob URL，不用 asset protocol — 對 SayIt 改進）

## Capabilities

`capabilities/` 目錄會分兩個 file 而不是一個（對 SayIt 改進）：

- `hud.json` — 給 HUD 視窗：window control、event listen/emit、無 SQL/store
- `dashboard.json` — 給 Dashboard 視窗：window control、event、SQL、store、HTTP

理由：HUD 不需要 access SQL 與 settings store（Rust 會 push events）。最小權限原則。

## Profile / Build settings

`Cargo.toml` `[profile.release]`（學 SayIt）：

```toml
panic = "abort"
codegen-units = 1
lto = true
opt-level = "s"   # 'z' if 還想更小
strip = true
```

## 環境變數

| Var | 用途 | Phase |
|---|---|---|
| `TAURI_DEV_HOST` | HMR for mobile/network testing | Phase 1+ |
| `VITE_SENTRY_DSN` | Frontend Sentry | Phase 2 |
| `VITE_SENTRY_ENVIRONMENT` | Frontend Sentry env | Phase 2 |
| `VITE_SENTRY_RELEASE` | Frontend Sentry release tag | Phase 2 |
| `VITE_SENTRY_SOURCEMAPS_ENABLED` | Vite sourcemap toggle | Phase 2 |
| `SENTRY_DSN` | Rust Sentry | Phase 2 |
| `SENTRY_ENVIRONMENT` | Rust Sentry env | Phase 2 |
| `TAURI_SIGNING_PRIVATE_KEY` | Updater minisign key | Phase 2 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 同上 password | Phase 2 |

## 連結

- 系統架構 → [`01-architecture.md`](01-architecture.md)
- Rust modules → [`03-rust-modules.md`](03-rust-modules.md)
- Frontend 結構 → [`04-frontend-structure.md`](04-frontend-structure.md)
- SayIt frontend deps reference → [`../reference/sayit-frontend-analysis.md#13-frontend-dependencies-packagejson`](../reference/sayit-frontend-analysis.md)
- SayIt backend deps reference → [`../reference/sayit-backend-analysis.md#2-cargo-dependencies`](../reference/sayit-backend-analysis.md)
