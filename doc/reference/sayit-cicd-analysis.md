# SayIt 建置/CI/發版/測試基礎建設分析報告

> **來源**：Opus subagent (general-purpose) 對 `C:\Users\lulub\Downloads\SayIt` 的 build、CI、release、testing infrastructure 分析
> **日期**：2026-05-02
> **分析範圍**：GitHub Actions、scripts、tests、Claude hooks、BMad workflow（不深入 src/ 與 src-tauri/src/ 程式碼）

## 1. Build Pipeline Overview

專案遵循標準 Tauri v2 lifecycle 但有幾個 disciplined elaborations。

**Local development flow：**

1. 開發者裝 Node 24（pin via `.nvmrc`）、pnpm 10.28.2（pin via `package.json` `packageManager` field）、Rust stable
2. `pnpm install --frozen-lockfile` 裝 JS deps（with `pnpm.onlyBuiltDependencies: ["esbuild"]` whitelist native build steps）
3. `pnpm tauri dev` start Vite dev server on port 1420 (`vite.config.ts` → `server.port: 1420, strictPort: true`) 並 launch Tauri shell pointing at it
4. Claude Code 編輯時，hooks at `.claude/hooks/` 自動 run `vue-tsc`、`rustfmt`、`eslint --fix` after every edit

**Build flow：**

- `pnpm build` 跑 `vue-tsc --noEmit && vite build`。Vite 設定為 **multi-entry HTML**（`index.html` for HUD + `main-window.html` for Dashboard）via `rollupOptions.input` (`vite.config.ts`)
- `vite.config.ts` 透過 `define` 注入 `__APP_VERSION__` 為 global（從 `package.json` source）
- Sourcemaps 條件式啟用 via env flag `VITE_SENTRY_SOURCEMAPS_ENABLED=true` — 只在 macOS arm64 release job set
- `pnpm tauri build` invoke Rust bundler。`tauri.conf.json` 宣告 `bundle.targets: "all"` 與 `bundle.createUpdaterArtifacts: true`（這是產生 `.sig` files for Tauri updater 的）

**Release flow：** Local script `scripts/release.sh` → 4 處 version bump → commit + tag + push → GitHub Actions `release.yml` trigger on `v*` tag → matrix builds 3 platforms → drafts a GitHub Release → `publish-release` job promotes draft to public。

## 2. GitHub Actions Workflows

只有兩個 workflow files 在 `.github/workflows/`：

### 2.1 `ci.yml` — Continuous Integration

Triggers：`push` to `main`、`pull_request` to `main`。

兩個 parallel jobs：

**Job `check`** (Ubuntu)：

- `actions/checkout@v4`
- `actions/setup-node@v4` reading `node-version-file: ".nvmrc"` (Node 24)
- `pnpm/action-setup@v4`
- `pnpm install --frozen-lockfile`
- Type check: `npx vue-tsc --noEmit`
- Run unit tests: `pnpm test`

**Job `rust-check`** (matrix `macos-latest` 與 `windows-latest`)：

- Toolchain via `dtolnay/rust-toolchain@stable`
- Cache via `swatinem/rust-cache@v2` with `workspaces: src-tauri`
- `cargo check` in `src-tauri/`

Notable：**CI 不跑 `cargo test`、ESLint、Playwright e2e、或 coverage。** 更豐富的 "verify" suite 在 `verify` skill (`.claude/skills/verify/SKILL.md`) 內、commits 前在本地跑而不是 CI。

### 2.2 `release.yml` — Cross-platform Release

Triggers：tag push matching `v*`，OR manual `workflow_dispatch` with `tag` input（特定 tag 可 on demand re-build）。

`permissions: contents: write`（為了 tag release 與 upload artifacts）。

**Job `build` matrix** (`fail-fast: false`)：

| platform | args | rust_target | stable_name | upload_sourcemaps |
|---|---|---|---|---|
| macos-latest | `--target aarch64-apple-darwin` | aarch64-apple-darwin | SayIt-mac-arm64.dmg | true |
| macos-latest | `--target x86_64-apple-darwin` | x86_64-apple-darwin | SayIt-mac-x64.dmg | false |
| windows-latest | "" | "" | SayIt-windows-x64.exe | false |

Steps in order：

1. Checkout
2. **Resolve release metadata** — bash script strip `refs/tags/` 與 `v` prefix、set `RELEASE_TAG`、`RELEASE_VERSION`、加 `SENTRY_RELEASE=sayit@<version>` 與 `VITE_SENTRY_RELEASE=sayit@<version>` 進 `$GITHUB_ENV`
3. setup-node + pnpm + Rust toolchain (with target) + Rust cache (workspace `./src-tauri -> target`)
4. `pnpm install --frozen-lockfile`
5. **`tauri-apps/tauri-action@v0`** — 工作主力。接 `tagName`、`releaseName: "SayIt v${version}"`、`releaseDraft: true`、`prerelease: false`、加 matrix `args`。Crucially、env 傳 13 secrets (見 Section 4)
6. **Sourcemap upload to Sentry** — 只在 `matrix.upload_sourcemaps == 'true'` (macOS arm64 only、避免重複 upload)。用 `npx @sentry/cli` (a) `releases new`、(b) `sourcemaps upload dist/assets --url-prefix "~/assets" --validate --wait`、(c) `releases finalize`。如果 `SENTRY_AUTH_TOKEN`/`SENTRY_ORG`/`SENTRY_PROJECT` 缺失 silent skip
7. **Stable-name asset upload (macOS)** — `find src-tauri/target -path "*/bundle/dmg/*.dmg"` 然後 `cp` to stable name、然後 `gh release upload "$RELEASE_TAG" "${stable_name}" --clobber`。這就是 `releases/latest/download/SayIt-mac-arm64.dmg` 有 permanent URL 的原因
8. **Stable-name asset upload (Windows)** — 用 PowerShell、find `*-setup.exe`、copy to stable name、`gh release upload --clobber`

**Job `publish-release`**（depend on `build`、跑在 Ubuntu）：re-resolve tag 並跑 `gh release edit "$RELEASE_TAG" --draft=false`。Public release 只在 3 platform builds 都 pass 後 finalize。

## 3. Release Process

### 3.1 The script: `scripts/release.sh`

Usage：`./scripts/release.sh 0.9.5`。Script 在允許 release 前 enforce strict invariants：

1. **Version format check** — must match `^[0-9]+\.[0-9]+\.[0-9]+$`
2. **CHANGELOG entry check** — `grep -q "## \[$VERSION\]" CHANGELOG.md`。缺失 exit `錯誤: CHANGELOG.md 缺少 v$VERSION 的紀錄`
3. **Clean working tree** — bail if `git status --porcelain` non-empty
4. **Tag uniqueness** — bail if `v$VERSION` tag 已存在
5. **Branch check** — refuse on detached HEAD（否則 `git push origin "$CURRENT_BRANCH"` 會 fail）

### 3.2 The 4-place version bump

Per CLAUDE.md 與 script confirm，version 必須在四個 files 同步：

1. `package.json` → `.version` (jq edit)
2. `src-tauri/tauri.conf.json` → `.version` (jq edit)
3. `src-tauri/Cargo.toml` → `version = "X.Y.Z"` line（Python in-place find/replace looking for *current* version string — fail fast if drift detected）
4. `src-tauri/Cargo.lock` → `name = "sayit"\nversion = "..."` block（Python in-place replace、再次 anchor on current version）

Script 然後 re-read 全部四個 assert 跟 `$VERSION` 一致、mismatch exit。

### 3.3 Commit, tag, push

```bash
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: bump version to $VERSION"
git tag "v$VERSION"
git push origin "$CURRENT_BRANCH"   # branch first
git push origin "v$VERSION"          # tag second — separated to avoid GitHub deduping the tag event
```

Branch-then-tag 分離是 intentional。v0.2.1 的 CHANGELOG 明確說 「新增 workflow_dispatch 觸發器並分離 tag 推送」 — push both refs 在一個 push command 可能 suppress tag-push event in Actions。

### 3.4 Tag → workflow → publish flow

Tag pushed → `release.yml` trigger → 3 matrix jobs build → 每個 upload platform-specific artifacts (DMG/MSI) 加 updater `.sig` files 加 stable-named assets to draft release → `publish-release` job flip draft=false → release public at `https://github.com/chenjackle45/SayIt/releases/v<version>` 與 `releases/latest/download/SayIt-mac-arm64.dmg` 等 resolve to new files。

## 4. Code Signing & Notarization

### 4.1 Apple (macOS)

`tauri.conf.json` 宣告 signing identity：`"signingIdentity": "Developer ID Application: Tai-Cheng Chen (G9J8D2T6DV)"` 加 `"entitlements": "./Entitlements.plist"`。

`tauri-action@v0` 消費 6 個 Apple-related env vars 做 certificate import + sign + notarize：

| Env | Source | Purpose |
|---|---|---|
| `APPLE_CERTIFICATE` | secret | Developer ID `.p12` Base64-encoded |
| `APPLE_CERTIFICATE_PASSWORD` | secret | `.p12` password |
| `APPLE_SIGNING_IDENTITY` | secret | Identity string match |
| `APPLE_ID` | secret | Apple ID email for notarization |
| `APPLE_PASSWORD` | secret | App-specific password |
| `APPLE_TEAM_ID` | secret | Team ID for notarytool |

`tauri-action` 自動處理 keychain creation、`codesign`、與 `xcrun notarytool submit --wait`。

### 4.2 Tauri updater signing

兩個 secrets：

- `TAURI_SIGNING_PRIVATE_KEY` — minisign private key（public key embed in `tauri.conf.json` → `plugins.updater.pubkey` as Base64）
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — passphrase

`bundle.createUpdaterArtifacts: true` 讓 Tauri 在每個 installer 旁產生 `.sig` companion、用 private key 簽。Frontend updater plugin 套用任何 update 之前用 embedded public key 驗 signatures。

`.gitignore` 包含 `*.key` 與 `*.key.pub` 防止 accidental commit local minisign material。

## 5. Auto-updater

**Endpoint：** `tauri.conf.json` → `plugins.updater.endpoints[0]` 是 `https://github.com/chenjackle45/SayIt/releases/latest/download/latest.json`。`latest.json` manifest 由 `tauri-action` auto-upload 到每個 release，所以 `releases/latest/download/...` 在 release public 後立刻 resolve to new manifest。

**Public key：** `pubkey: "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDBENjAzMzYyRTMxMTNGQjgKUldTNFB4SGpZak5nRFJoYUpxR0RGMkZyZUhHYStyNzgxWEwxVHR2ZEFZMzh3eFQ1QlpXVEJLVGUK"` (minisign public key、base64-wrapped)。

**Frontend integration**（per CLAUDE.md 與 `tests/unit/auto-updater.test.ts`）：

- `src/lib/autoUpdater.ts` export `checkForAppUpdate()`、`downloadUpdate()`、`installAndRelaunch()`、`downloadInstallAndRelaunch()`
- `checkForAppUpdate()` 回傳 `Promise<UpdateCheckResult>` typed as `up-to-date` | `update-available` | `error`
- `installAndRelaunch()` 透過 `invoke` 呼叫 Rust 的 `request_app_restart` command（而非 Tauri 內建 `relaunch`）— v0.6.0 修復 `_exit(0)` 殺掉 Tauri 的 restart logic 的 issue
- Auto-check schedule：`main-window.ts` 在 launch 後 5 秒首次 check、然後 `setInterval` 每 4 小時
- Manual check from Sidebar Footer button 用 `useFeedbackMessage` 顯示結果
- Known limitation：`window.confirm` 在 Tauri WKWebView on macOS 被 silently 忽略 — 需要 in-app UI rework

## 6. Sentry Integration

### 6.1 Architecture

- **Frontend**：`@sentry/vue` ^10.42.0。兩個 windows 分別初始化：
  - HUD：`initSentryForHud(app)` — light、無 `browserTracingIntegration`
  - Dashboard：`initSentryForDashboard(app, router)` — full、with router integration
  - 都 tag `window: "hud" | "dashboard"` 做 source disambiguation
- **Backend**：`sentry` 0.46 Rust crate、用 guard pattern 在 `lib.rs::run()` 初始化 (`let _sentry_guard = sentry::init(...)` bound to function-local 活到 process exit)
- 只在 DSN env var non-empty 且不是 `__`-prefixed CI placeholder 時 enable
- `send_default_pii: false` on Rust side

### 6.2 Release naming convention (hard rule)

Per CLAUDE.md「發版硬規則」：production Sentry release **永遠** 是 `sayit@<version>`、由 `release.yml` 排他設定。Frontend 與 Rust 不可 manually pick different names。Workflow 注入 identical `SENTRY_RELEASE` 與 `VITE_SENTRY_RELEASE` env vars from single resolved tag、`tauri-action` build step 收到為 compile-time/runtime constants。

### 6.3 Sourcemap upload

只 **macOS arm64 matrix entry** 有 `upload_sourcemaps: "true"`。Dedicated step 跑 `npx @sentry/cli`：

1. `releases new "$SENTRY_RELEASE" || true` (idempotent)
2. `sourcemaps upload dist/assets --release "$SENTRY_RELEASE" --url-prefix "~/assets" --validate --wait`
3. `releases finalize "$SENTRY_RELEASE" || true`

如果 `SENTRY_AUTH_TOKEN`/`SENTRY_ORG`/`SENTRY_PROJECT` 任一缺失整個 skip — graceful degradation rather than failure。

Hard rule「正式版 telemetry 與 sourcemap upload 只能走 release.yml，不得繞過 workflow 手動上傳」是強制 single authoritative source for sourcemaps per release。

### 6.4 Coverage in app

56 `captureError(err, { source, step })` 呼叫點 across stores 與 bootstrap files (`useVoiceFlowStore` 17、`useSettingsStore` 11、`useHistoryStore` 8 等)。Layer rule：只 stores 與 entry scripts 呼叫 `captureError`；`lib/` 只 throw 從不 report。

## 7. Testing Strategy

### 7.1 Unit tests (Vitest)

`vitest.config.ts`：

- `globals: true`、`environment: "jsdom"`
- `include: ["tests/unit/**/*.test.ts", "tests/component/**/*.test.ts"]`
- Coverage provider：`v8`、`include: ["src/**/*.ts", "src/**/*.vue"]`、exclude `main.ts`/`main-window.ts`/`*.d.ts`
- Alias `@` → `/src` mirror `tsconfig.json` `paths`

**Coverage breadth**（16 unit test files in `tests/unit/`）：

- `auto-updater.test.ts`、`enhancer.test.ts`、`error-utils.test.ts`、`hallucination-detector.test.ts`、`i18n-settings.test.ts`、`llmProvider.test.ts`、`settingsStore.test.ts`、`use-history-store.test.ts`、`use-settings-store.test.ts`、`use-settings-store-autostart.test.ts`、`use-vocabulary-store.test.ts`、`use-voice-flow-store.test.ts`（33 KB — 最大、voice-flow 是 orchestrator）、`format-utils.test.ts`、`api-pricing.test.ts`、`factories.test.ts`、`types.test.ts`

**Mocking style**（從 `auto-updater.test.ts`）：

- `vi.mock("@tauri-apps/plugin-updater", () => ({ check: mockCheck }))` to stub Tauri plugin functions
- `vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }))` for IPC
- `vi.resetModules()` in `beforeEach`、dynamic `await import("../../src/lib/autoUpdater")` 每個 test 拿到 fresh module
- Console silenced via `vi.spyOn(console, ...)`
- Test names tag with priority：`[P0]`、`[P1]`、`[P2]`、`[P3]`。Given/When/Then comments structure 每個

### 7.2 Component tests (Vitest + @vue/test-utils)

3 files in `tests/component/`：

- `NotchHud.test.ts` — render HUD component、mock `@tauri-apps/api/event` 的 `listen`/`emit`、supply minimal `vue-i18n` `createI18n` plugin 滿足 `<i18n-t>` lookups、assert classes like `.waveform-container`/`.elapsed-timer`
- `AccessibilityGuide.test.ts`
- `i18n-smoke.test.ts`

### 7.3 E2E tests (Playwright)

`playwright.config.ts`：

- `testDir: "./tests/e2e"`、`outputDir: "./test-results"`
- `fullyParallel: true`、`retries: 2 in CI / 0 local`、`workers: 1 in CI / undefined local`
- Reporters：`html`（`playwright-report` folder、`open: "never"`）+ `list`
- Timeouts：`60_000` test / `10_000` expect / `15_000` action / `30_000` navigation
- Trace `on-first-retry`、screenshots `only-on-failure`、video `retain-on-failure`
- `webServer.command: "pnpm dev"`、`url: http://localhost:1420`、`reuseExistingServer: !CI`、120s startup timeout

**Tauri 怎麼為 E2E 啟動：** 沒有 really。Playwright drive `pnpm dev`（只 Vite dev server）on `localhost:1420`、test against web frontend。目前只有一個 E2E test (`tests/e2e/smoke.test.ts`)、navigate to `/`、wait for `domcontentloaded`、assert `expect(page).toHaveTitle(/whisper/i)`。BMad `framework-setup-progress.md` 與 `automation-summary.md` 明確 defer Tauri-runtime E2E 因 CI complexity (「E2E 擴展需要 Tauri runtime + 專用 CI 環境」)。

### 7.4 Rust tests

BMad automation summary 記錄 14 Rust inline tests 在 scaffolding 時加入：

- `src-tauri/src/plugins/clipboard_paste.rs` — 9 tests on `ClipboardError` Display/Serialize/Debug
- `src-tauri/src/lib.rs` — 5 tests on extracted `calculate_centered_window_x()` pure function
- 後續加：`audio_recorder.rs` (9 tests)、`transcription.rs` (4 tests)

透過 `cd src-tauri && cargo test` run。目前不在 CI（只 `cargo check` runs）。

### 7.5 Coverage tooling

`@vitest/coverage-v8` ^4.0.18。`pnpm test:coverage` runs `vitest run --coverage`。沒設 coverage thresholds；CI 沒 Codecov upload step。Coverage 只 informational。

### 7.6 Test support

`tests/support/`：

- `factories/` — Faker-based factories for `TranscriptionRecord` 與 `VocabularyEntry`。Pattern：typed export + `create*(overrides: Partial<...>)`。Default values 用 `faker.lorem`、`faker.string.uuid`、`faker.number.int` 等。Tests 用 `[P1]` priority tagging
- `fixtures/index.ts` — 目前 placeholder 只 re-export `test as base, expect` from `@playwright/test`。Comment 說明 future plan to merge with `@seontechnologies/playwright-utils` fixtures

## 8. `.claude/` Hooks

`.claude/settings.json` 定義兩個 phases：

**PreToolUse**（matcher `Edit|Write`）：

- `protect-config.sh` (timeout 5s)

**PostToolUse**（matcher `Edit|Write`、in order）：

- `typecheck.sh` (timeout 30s)
- `rustfmt.sh` (timeout 10s)
- `eslint.sh` (timeout 15s)

所有 hooks read JSON from stdin parse `tool_input.file_path` via `jq`。

### 8.1 `protect-config.sh` (PreToolUse)

Two-tier protection：

- **Hard block (exit 2)**：`Cargo.lock`、`pnpm-lock.yaml`、`package-lock.json`、`yarn.lock` — emit JSON error reason "Lock 檔由套件管理工具自動產生，禁止手動修改"
- **Soft warning (exit 0 with stdout)**：`tauri.conf.json` 與 `Cargo.toml` — print warning 但允許 edit
- Other：silent pass

這是防止 Claude 透過善意 edits 破壞 lockfiles 的機制。

### 8.2 `typecheck.sh` (PostToolUse)

- Trigger only on `*.ts` 與 `*.vue`
- Skip `*.test.ts`、`*.spec.ts`、`*.d.ts`
- Run `npx vue-tsc --noEmit 2>&1 | head -30`
- Exit 1 (not 2) on failure — non-blocking、回報 errors 給 Claude

### 8.3 `rustfmt.sh` (PostToolUse)

- Trigger only on `*.rs`
- Run `rustfmt "$FILE_PATH"` directly (in-place)
- Exit 1 on failure (non-blocking)

### 8.4 `eslint.sh` (PostToolUse)

- Trigger only on `*.ts` 與 `*.vue`
- **Skip `*/components/ui/*`** — shadcn-vue generated components 不 lint
- Run `npx eslint --fix "$FILE_PATH"` 並 pipe first 20 lines on failure
- Exit 1 on failure (non-blocking)

## 9. `.claude/agents/` — Custom Subagents

一個 subagent：`tauri-reviewer.md`。

**Role**：Tauri IPC consistency reviewer。Read-only（`Read`、`Grep`、`Glob` only — 明確 forbidden 修改 files）。

**Audit scope**：

1. Command registration completeness — `#[tauri::command]` ↔ `tauri::generate_handler![]` in `lib.rs` ↔ frontend `invoke('cmd')`
2. Command signature alignment — Rust snake_case ↔ frontend camelCase、return types、`Result<T,E>` mapping
3. Event name consistency — Rust `emit("event-name", payload)` ↔ constants in `src/composables/useTauriEvents.ts` ↔ frontend `listenToEvent(EVENT_CONSTANT, cb)`
4. Payload type alignment — Rust `#[serde(rename_all = "camelCase")]` struct fields ↔ TypeScript interfaces。Type mapping rules 包含 (`Option<T>` → `T | null`、`i32`/`i64`/`f64` → `number`)

**Output format**：`[PASS]/[WARN]/[FAIL]` per check、終止 ASCII summary table。

**Execution recipe**：9 explicit steps、start with parsing `generate_handler![]`、grep `#[tauri::command]`/`invoke(`/`emit(`/`listenToEvent`、然後 cross-compare — codified rather than ad-hoc。

Wire 進 `.claude/skills/ipc-review/SKILL.md` 為 slash command invocation。

## 10. `.agents/` Directory

`.agents/skills/` 是不同 slot — 它 hold **48+ BMad-method agent skill folders**（每個含 SKILL.md），由 BMad installer 安裝。範例：

- `bmad-agent-bmm-dev`、`bmad-agent-bmm-pm`、`bmad-agent-bmm-architect`、`bmad-agent-bmm-qa`、`bmad-agent-bmm-sm` 等
- Workflow skills：`bmad-bmm-create-prd`、`bmad-bmm-create-architecture`、`bmad-bmm-create-story`、`bmad-bmm-dev-story`、`bmad-bmm-code-review` 等
- Test architecture (TEA module)：`bmad-tea-testarch-atdd`、`bmad-tea-testarch-automate`、`bmad-tea-testarch-ci` 等

每個 `.claude/commands/bmad-*.md`（51 files）是 thin slash-command stub、形式為「load `_bmad/{module}/agents/{agent}.md` 並 follow activation block」或「load `_bmad/core/tasks/workflow.xml` 並 pass `_bmad/{module}/workflows/.../workflow.yaml` as `workflow-config`」。`.agents/skills/...` folders 是 commands 展開的實際 instruction bodies。

## 11. BMad-method Usage

Repo 用 **BMad-method 6.0.4** bootstrapped（見 `_bmad/_config/manifest.yaml`）：

```yaml
installation:
  version: 6.0.4
  installDate: 2026-02-24
modules:
  - name: core (built-in, v6.0.4)
  - name: bmm  (built-in, v6.0.4)              # build/manage
  - name: tea  (external, v1.4.1, npm: bmad-method-test-architecture-enterprise)
ides: [claude-code, codex]
```

User identity (`_bmad/_memory/config.yaml`)：user `Jackle`、communication & doc language `繁體中文`、output folder `_bmad-output`。

### 11.1 `_bmad/` — workflow definitions

- `_config/` — manifests
- `core/` — built-in agents/tasks/workflows
- `bmm/` — Build/Manage Module
- `tea/` — Test Architecture Enterprise
- `_memory/` — user config

### 11.2 `_bmad-output/project-context.md` — the rule book

70 KB 文件含 **323 rules**（frontmatter `rule_count: 323`）。22 sections cover：

- Technology stack & versions
- Frontend dependencies + 「已安裝但不應使用」list
- Tauri plugins matrix
- External APIs
- Sentry integration architecture
- Language rules
- Critical rules per layer
- Testing rules
- Code quality、workflow rules、critical rules
- Per-feature contracts

### 11.3 `_bmad-output/planning-artifacts/`

- `product-brief-sayit-2026-02-28.md` — 10 KB
- `prd.md` — 24 KB
- `architecture.md` — 42 KB
- `ux-ui-design-spec.md` — 35 KB
- `epics.md` — 47 KB
- `implementation-readiness-report-2026-03-01.md` — 14 KB
- `sprint-change-proposal-2026-03-15.md` — 18 KB

### 11.4 `_bmad-output/implementation-artifacts/`

19 story `.md` 加 14 tech-spec docs。Story files 命名為 `{epic}-{story}-{slug}.md`。每個 story 13–35 KB。

### 11.5 `_bmad-output/test-artifacts/`

- `framework-setup-progress.md`
- `automation-summary.md`

## 12. `package.json` Scripts

```json
"dev":           "vite",
"build":         "vue-tsc --noEmit && vite build",
"preview":       "vite preview",
"tauri":         "tauri",
"test":          "vitest run",
"test:watch":    "vitest",
"test:coverage": "vitest run --coverage",
"test:e2e":      "playwright test",
"test:e2e:ui":   "playwright test --ui"
```

注意 absences：沒 `lint` script、沒 `format` script、沒 `release` script wrapper（直接呼叫 `./scripts/release.sh`）。Tauri CLI 透過 `pnpm tauri ...` expose via `"tauri": "tauri"` 因為 `@tauri-apps/cli` 是 devDependency。

## 13. Pre-commit / Git Hooks

**沒有 Git hooks** 設定（沒 `husky`、`simple-git-hooks`、`lint-staged`、或 `.husky/` directory）。Developer-side enforcement 來自兩個地方：

1. Claude Code hooks in `.claude/hooks/`（只在 Claude 編輯時 fire — 不在 `git commit`）
2. `verify` skill (`.claude/skills/verify/SKILL.md`) 是 manually invoked slash command run 5 commands sequentially：
   - `npx eslint .`
   - `npx vue-tsc --noEmit`
   - `pnpm test`
   - `cd src-tauri && cargo clippy -- -D warnings`
   - `cd src-tauri && cargo check`

CI 提供 safety net (`vue-tsc --noEmit`、`pnpm test`、`cargo check`) 但 **不** 包含 `eslint` 或 `cargo clippy`。

## 14. Environment Management

- **`.nvmrc`**：single line `24` — Node 24 LTS、被 `actions/setup-node` (`node-version-file: ".nvmrc"`) 與 developer tooling (nvm、fnm、Volta) 消費
- **`pnpm-lock.yaml`**：181 KB、committed。CI 用 `pnpm install --frozen-lockfile`
- **`pnpm-workspace.yaml`**：only `onlyBuiltDependencies: esbuild` — duplicate `package.json` `pnpm.onlyBuiltDependencies` setting
- **`.gitignore`** ignore：`node_modules/`、`dist/`、`src-tauri/target/`、`.env*`、`.vscode/`、`.idea/`、`.DS_Store`、`*.log`、`test-results/`、`playwright-report/`、`playwright/.auth/`、`.claude/settings.local.json`、`*.key`、`*.key.pub`
- **`tsconfig.json`** 嚴格：`target ES2021`、`strict: true`、`noUnusedLocals: true`、`noUnusedParameters: true`、`noFallthroughCasesInSwitch: true`、`allowImportingTsExtensions: true`、`isolatedModules: true`、`moduleDetection: "force"`、`noEmit: true`。Path alias `@/* → ./src/*`
- **`eslint.config.js`** 是 flat config

## 15. Issue / PR Templates

**無 present**。`.github/` 只 contain `workflows/`。沒 `ISSUE_TEMPLATE/`、沒 `PULL_REQUEST_TEMPLATE.md`、沒 `dependabot.yml`、沒 `CODEOWNERS`。Solo developer setup。

## 16. Notable Patterns

1. **Stable-name asset uploading** — Beyond Tauri 自動 versioned artifacts、每個 platform job 跑額外 `gh release upload --clobber` push canonical filenames。配 `releases/latest/download/<filename>` semantics、給 marketing site permanent links 不會 break across versions

2. **Branch + tag pushed separately** — v0.2.1 修復 GitHub Actions issue（push branch 與 tag together suppress tag-push event）。release.sh script enforce two-step push

3. **Rule duplication in 4 places per release** — Version sync across `package.json`、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock` 痛苦但必要因為 Cargo package version baked into Rust binaries。Script 的 Python in-place replace 是 clever pattern：anchor on *current* version string 並 fail loudly if mismatch — optimistic concurrency check catch drift

4. **Sentry release name as a hard rule** — Workflow 從 single resolved tag 注入 identical `SENTRY_RELEASE` 與 `VITE_SENTRY_RELEASE` env vars。Codified as「正式版 Sentry release 一律由 release.yml 產生」

5. **Sourcemaps uploaded only once** — 即使 3 matrix builds、只 macOS arm64 upload sourcemaps (`upload_sourcemaps: "true"` 只在那個 matrix entry set)。避免重複 Sentry artifacts 與 bandwidth waste

6. **Conditional sourcemap generation in vite.config.ts** — `build.sourcemap: shouldGenerateSentrySourcemaps` (drive by `VITE_SENTRY_SOURCEMAPS_ENABLED`)。Local dev builds 與其他 platform CI builds 完全 skip 產生 sourcemaps — 只那個 upload 的 job 才產生

7. **Two-phase Sentry init for two windows** — `initSentryForHud`（lightweight）vs `initSentryForDashboard`（full with tracing）。每個 window 在自己 entry script 內 initialize — multi-entry Vite setup 讓 HUD 與 Dashboard 是分開 JS bundles

8. **Read-only IPC reviewer subagent** — `tauri-reviewer.md` 明確說「你只能使用唯讀工具：Read、Grep、Glob。不可修改任何檔案」。Sandbox auditor pattern

9. **Hooks treat blocking vs. non-blocking distinctly** — `protect-config.sh` 用 exit code 2 (Claude Code 的 hard-block convention) actually 防止 edits、`typecheck.sh`/`rustfmt.sh`/`eslint.sh` 用 exit 1 report-but-allow — Claude 看到 errors 在下一個 turn 自我修正

10. **No husky/lint-staged** — Hooks 在兩 layer：agent-time (Claude Code hooks during edits) 與 CI-time (workflows on push/PR)。`verify` skill 是 manual pre-commit checklist

11. **BMad-method drives the docs** — 323-rule `project-context.md` 是 source of truth；CLAUDE.md 是 curated quick-reference subset；subdomain docs (PRD、architecture、UX spec、epics、tech specs per feature、story files) 在 `_bmad-output/` 由 BMad slash-commands 產生

12. **Multi-entry Vite is what the dual-window architecture rests on** — `rollupOptions.input` 宣告 both `index.html` (HUD) 與 `main-window.html` (Dashboard) as parallel build entries、each with its own `main.ts`/`main-window.ts`

13. **Test priority tagging convention** — `[P0]`/`[P1]`/`[P2]`/`[P3]` prefixes in `it()` names map to BMad TEA `risk_threshold: p1` priority matrix

14. **Faker-backed factories instead of fixtures** — Tests import `createTranscriptionRecord({ wasEnhanced: true, ... })` rather than read JSON fixtures。Random IDs 避免 parallel-test collisions；explicit overrides 表達 intent。Type-safe `Partial<T>` overrides enforce schema correctness at test-write time
