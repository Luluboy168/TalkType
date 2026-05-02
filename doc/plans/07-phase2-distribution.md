# Phase 2：發行 / 簽名 / CI 規劃

> **狀態**：Draft v1（Phase 2 才執行）
> **最後更新**：2026-05-02
> **前提**：Phase 1 結束 = clone + build 可 work，Phase 2 = 從這裡進化到 production app

學 SayIt 的 release pipeline，但對幾個地方做改進。

## Phase 2 發行 Pipeline 總覽

```
   Local: ./scripts/release.sh 0.X.0
       │ (bump version 4 places + commit + tag + push)
       ↓
   GitHub Actions: release.yml triggers on v* tag
       │
       ├── matrix [windows-x64, macos-arm64, macos-x64]
       │     │
       │     ├── checkout
       │     ├── Resolve metadata (RELEASE_TAG, RELEASE_VERSION, SENTRY_RELEASE)
       │     ├── setup-node + pnpm + Rust + cache
       │     ├── pnpm install --frozen-lockfile
       │     ├── tauri-action@v0
       │     │     ├── pnpm build (vue-tsc + vite)
       │     │     ├── pnpm tauri build --target ...
       │     │     ├── (macOS) codesign + notarize
       │     │     └── upload to draft GitHub Release
       │     ├── (only macos-arm64) Sentry sourcemap upload
       │     └── Stable-name asset upload (TalkType-{platform}-{arch}.{ext})
       │
       └── publish-release job (depends on all matrix jobs)
              ├── gh release edit --draft=false
              └── public release available
```

## 4-place Version Sync

學 SayIt：

```bash
#!/usr/bin/env bash
# scripts/release.sh
set -euo pipefail

VERSION="$1"

# 1. Validate format
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Bad version"; exit 1; }

# 2. CHANGELOG entry exists
grep -q "## \[$VERSION\]" CHANGELOG.md || { echo "Missing CHANGELOG entry"; exit 1; }

# 3. Clean tree
[[ -z "$(git status --porcelain)" ]] || { echo "Working tree dirty"; exit 1; }

# 4. Tag uniqueness
git rev-parse "v$VERSION" 2>/dev/null && { echo "Tag exists"; exit 1; }

# 5. Branch check (no detached HEAD)
CURRENT_BRANCH=$(git symbolic-ref --short HEAD)

# 6. Bump 4 places
jq ".version = \"$VERSION\"" package.json > package.json.tmp && mv package.json.tmp package.json
jq ".version = \"$VERSION\"" src-tauri/tauri.conf.json > src-tauri/tauri.conf.json.tmp && mv src-tauri/tauri.conf.json.tmp src-tauri/tauri.conf.json

CURRENT_CARGO_VERSION=$(grep -oP '^version = "\K[^"]+' src-tauri/Cargo.toml | head -1)
python3 -c "
import re
with open('src-tauri/Cargo.toml') as f: content = f.read()
new = re.sub(r'^version = \"$CURRENT_CARGO_VERSION\"\$', 'version = \"$VERSION\"', content, flags=re.M, count=1)
if new == content: raise SystemExit('Cargo.toml version not found')
with open('src-tauri/Cargo.toml', 'w') as f: f.write(new)
"

python3 -c "
import re
with open('src-tauri/Cargo.lock') as f: content = f.read()
pattern = r'(\\[\\[package\\]\\]\\nname = \"talktype\"\\nversion = \")$CURRENT_CARGO_VERSION(\")'
new = re.sub(pattern, r'\\1$VERSION\\2', content)
if new == content: raise SystemExit('Cargo.lock entry not found')
with open('src-tauri/Cargo.lock', 'w') as f: f.write(new)
"

# 7. Commit + tag + push (separately)
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: bump version to $VERSION"
git tag "v$VERSION"
git push origin "$CURRENT_BRANCH"
git push origin "v$VERSION"  # 分開 push 避免 GitHub dedupe tag event

echo "✅ Version $VERSION released. CI will build and publish."
```

## GitHub Actions：`ci.yml`（Phase 1 already 有，Phase 2 加東西）

### Phase 2 補強

```yaml
name: CI

on:
  push: { branches: [main] }
  pull_request: { branches: [main] }

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version-file: .nvmrc
          cache: pnpm
      - uses: pnpm/action-setup@v4
      - run: pnpm install --frozen-lockfile
      - run: npx vue-tsc --noEmit
      - run: pnpm test
      - run: pnpm test:e2e  # Phase 2 加（需要 vite dev server）

  rust-check:
    strategy:
      matrix:
        os: [windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: swatinem/rust-cache@v2
        with: { workspaces: src-tauri }
      - run: cargo check
        working-directory: src-tauri
      - run: cargo clippy -- -D warnings  # Phase 2 加
        working-directory: src-tauri
      - run: cargo test  # Phase 2 加
        working-directory: src-tauri
```

## GitHub Actions：`release.yml`

```yaml
name: Release

on:
  push:
    tags: ['v*']
  workflow_dispatch:
    inputs:
      tag:
        description: Tag to build (e.g., v0.2.0)
        required: true

permissions:
  contents: write

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: windows-latest
            args: ''
            rust_target: ''
            stable_name: TalkType-windows-x64.exe
            upload_sourcemaps: 'false'
          - platform: macos-latest
            args: --target aarch64-apple-darwin
            rust_target: aarch64-apple-darwin
            stable_name: TalkType-mac-arm64.dmg
            upload_sourcemaps: 'true'
          - platform: macos-latest
            args: --target x86_64-apple-darwin
            rust_target: x86_64-apple-darwin
            stable_name: TalkType-mac-x64.dmg
            upload_sourcemaps: 'false'
    runs-on: ${{ matrix.platform }}
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Resolve release metadata
        shell: bash
        run: |
          if [[ "${{ github.event_name }}" == "workflow_dispatch" ]]; then
            TAG="${{ inputs.tag }}"
          else
            TAG="${GITHUB_REF#refs/tags/}"
          fi
          VERSION="${TAG#v}"
          echo "RELEASE_TAG=$TAG" >> $GITHUB_ENV
          echo "RELEASE_VERSION=$VERSION" >> $GITHUB_ENV
          echo "SENTRY_RELEASE=talktype@$VERSION" >> $GITHUB_ENV
          echo "VITE_SENTRY_RELEASE=talktype@$VERSION" >> $GITHUB_ENV
      
      - uses: actions/setup-node@v4
        with:
          node-version-file: .nvmrc
          cache: pnpm
      - uses: pnpm/action-setup@v4
      
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.rust_target }}
      
      - uses: swatinem/rust-cache@v2
        with:
          workspaces: ./src-tauri -> target
      
      - run: pnpm install --frozen-lockfile
      
      - uses: tauri-apps/tauri-action@v0
        env:
          # Tauri updater
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          # Apple
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
          # Sentry (compile-time bake)
          SENTRY_DSN: ${{ secrets.SENTRY_DSN }}
          VITE_SENTRY_DSN: ${{ secrets.VITE_SENTRY_DSN }}
          VITE_SENTRY_SOURCEMAPS_ENABLED: ${{ matrix.upload_sourcemaps }}
        with:
          tagName: ${{ env.RELEASE_TAG }}
          releaseName: TalkType v${{ env.RELEASE_VERSION }}
          releaseDraft: true
          prerelease: false
          args: ${{ matrix.args }}
      
      - name: Sentry sourcemap upload (macos-arm64 only)
        if: matrix.upload_sourcemaps == 'true'
        env:
          SENTRY_AUTH_TOKEN: ${{ secrets.SENTRY_AUTH_TOKEN }}
          SENTRY_ORG: ${{ secrets.SENTRY_ORG }}
          SENTRY_PROJECT: ${{ secrets.SENTRY_PROJECT }}
        run: |
          if [[ -z "$SENTRY_AUTH_TOKEN" || -z "$SENTRY_ORG" || -z "$SENTRY_PROJECT" ]]; then
            echo "Sentry credentials missing — skipping"
            exit 0
          fi
          npx @sentry/cli releases new "$SENTRY_RELEASE" || true
          npx @sentry/cli sourcemaps upload dist/assets \
            --release "$SENTRY_RELEASE" \
            --url-prefix "~/assets" \
            --validate --wait
          npx @sentry/cli releases finalize "$SENTRY_RELEASE" || true
      
      - name: Stable-name asset upload (macOS)
        if: runner.os == 'macOS'
        env: { GH_TOKEN: ${{ secrets.GITHUB_TOKEN }} }
        shell: bash
        run: |
          DMG=$(find src-tauri/target -path '*/bundle/dmg/*.dmg' | head -1)
          cp "$DMG" "${{ matrix.stable_name }}"
          gh release upload "$RELEASE_TAG" "${{ matrix.stable_name }}" --clobber
      
      - name: Stable-name asset upload (Windows)
        if: runner.os == 'Windows'
        env: { GH_TOKEN: ${{ secrets.GITHUB_TOKEN }} }
        shell: pwsh
        run: |
          $exe = Get-ChildItem -Recurse -Path "src-tauri/target" -Filter "*-setup.exe" | Select-Object -First 1
          Copy-Item $exe.FullName "${{ matrix.stable_name }}"
          gh release upload "$env:RELEASE_TAG" "${{ matrix.stable_name }}" --clobber

  publish-release:
    needs: [build]
    runs-on: ubuntu-latest
    steps:
      - name: Resolve tag
        shell: bash
        run: |
          if [[ "${{ github.event_name }}" == "workflow_dispatch" ]]; then
            echo "RELEASE_TAG=${{ inputs.tag }}" >> $GITHUB_ENV
          else
            echo "RELEASE_TAG=${GITHUB_REF#refs/tags/}" >> $GITHUB_ENV
          fi
      - name: Promote draft to public
        env: { GH_TOKEN: ${{ secrets.GITHUB_TOKEN }} }
        run: gh release edit "$RELEASE_TAG" --draft=false
```

## Code Signing

### Windows

#### Phase 2.1：先自簽（warning ok）

- 用 Visual Studio cert / OpenSSL 自製
- SmartScreen 會 warning「Unknown publisher」
- README 寫「目前自簽，您將看到 Windows SmartScreen 警告，請按『仍要執行』」
- 對 OSS 使用者 acceptable

#### Phase 2.2：申請 SignPath OSS Cert（免費）

- https://signpath.io/open-source/
- 對 OSS project 提供免費 OV cert
- 需要：active GitHub repo、CI 設定 SignPath
- 流程：他們 review → approval → set up CI integration → automatic signing on tag

#### Phase 2.3 (可選)：購買 EV Cert

- 完全去除 SmartScreen warning（甚至首次 install 也不會）
- 成本：~$300-500/年
- 不必須、看採用度決定

### macOS

#### 必要：Apple Developer Program

- $99/年
- 申請 Developer ID Application certificate
- 用 `security` 工具 export 為 `.p12` Base64-encoded → GitHub Secret `APPLE_CERTIFICATE`

#### Notarization

- App-Specific Password（從 Apple ID 設定建立）→ Secret `APPLE_PASSWORD`
- Apple ID email → Secret `APPLE_ID`
- Team ID → Secret `APPLE_TEAM_ID`
- `tauri-action` 自動處理 `xcrun notarytool submit --wait`

#### Entitlements

`src-tauri/Entitlements.plist`：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.device.audio-input</key>
    <true/>
    <key>com.apple.security.automation.apple-events</key>
    <true/>
</dict>
</plist>
```

#### Info.plist

```xml
<key>NSMicrophoneUsageDescription</key>
<string>TalkType needs microphone access to record audio for speech-to-text transcription.</string>
```

## Tauri Updater

### 產生 minisign keypair

```bash
# 一次性，本地跑：
npx @tauri-apps/cli signer generate -w ~/.tauri/talktype.key
# 輸出：
#   private key → ~/.tauri/talktype.key
#   public key  → ~/.tauri/talktype.key.pub
#   passphrase  → 你輸入的
```

**🔒 立刻把 private key 與 passphrase 備份到 1Password / Bitwarden / 私人 git repo。** 遺失 = 永遠不能再簽 update。

### 設定 GitHub Secrets

```
TAURI_SIGNING_PRIVATE_KEY = (cat ~/.tauri/talktype.key)
TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (你的 passphrase)
```

### tauri.conf.json

```json
"plugins": {
  "updater": {
    "active": true,
    "endpoints": [
      "https://github.com/Luluboy168/TalkType/releases/latest/download/latest.json"
    ],
    "dialog": false,  // 我們用 in-app UI 不用內建 dialog
    "pubkey": "(public key base64 from talktype.key.pub)"
  }
},
"bundle": {
  "createUpdaterArtifacts": true
}
```

### 前端 update flow

```typescript
// src/lib/autoUpdater.ts
import { check } from '@tauri-apps/plugin-updater';
import { invoke } from '@tauri-apps/api/core';

export async function checkForAppUpdate(): Promise<UpdateCheckResult> {
  try {
    const update = await check();
    if (update?.available) {
      return { status: 'update-available', version: update.version, available: update };
    }
    return { status: 'up-to-date' };
  } catch (e) {
    return { status: 'error', error: String(e) };
  }
}

export async function downloadInstallAndRelaunch(update: any): Promise<void> {
  await update.downloadAndInstall();
  await invoke('request_app_restart');
}
```

### 自動 check schedule

```typescript
// main-window.ts bootstrap
setTimeout(() => checkForAppUpdate(), 5000);   // 5s after launch
setInterval(() => checkForAppUpdate(), 4 * 60 * 60 * 1000);  // every 4h
```

## Sentry Integration

### Sentry 設定步驟

1. 註冊 Sentry account（免費 tier 5K errors/month 夠用）
2. 建 project：`talktype` (platform: `tauri-rust`)
3. 取得 DSN（每個 project 一個）
4. 取得 Auth token（用於 sourcemap upload）

### GitHub Secrets

```
SENTRY_DSN = (Rust Sentry DSN)
VITE_SENTRY_DSN = (Frontend Sentry DSN，可同 SENTRY_DSN 或分開 project)
SENTRY_AUTH_TOKEN = (sentry-cli token)
SENTRY_ORG = (org slug)
SENTRY_PROJECT = talktype
```

### Frontend `src/lib/sentry.ts`

```typescript
import * as Sentry from '@sentry/vue';

export function initSentryForHud(app: any) {
  if (!import.meta.env.VITE_SENTRY_DSN) return;
  Sentry.init({
    app,
    dsn: import.meta.env.VITE_SENTRY_DSN,
    environment: import.meta.env.VITE_SENTRY_ENVIRONMENT || 'production',
    release: import.meta.env.VITE_SENTRY_RELEASE || `talktype@${__APP_VERSION__}`,
    initialScope: { tags: { window: 'hud' } },
    tracesSampleRate: 0,
    sendDefaultPii: false,
  });
}

export function initSentryForDashboard(app: any, router: any) {
  if (!import.meta.env.VITE_SENTRY_DSN) return;
  Sentry.init({
    app,
    dsn: import.meta.env.VITE_SENTRY_DSN,
    environment: import.meta.env.VITE_SENTRY_ENVIRONMENT || 'production',
    release: import.meta.env.VITE_SENTRY_RELEASE || `talktype@${__APP_VERSION__}`,
    integrations: [Sentry.browserTracingIntegration({ router })],
    initialScope: { tags: { window: 'dashboard' } },
    tracesSampleRate: 0.1,
    sendDefaultPii: false,
  });
}

export function captureError(error: unknown, context?: Record<string, any>) {
  Sentry.captureException(error, { extra: context });
}
```

### Rust Sentry init

```rust
fn init_sentry() -> Option<sentry::ClientInitGuard> {
    let dsn = option_env!("SENTRY_DSN")?;
    if dsn.is_empty() || dsn.starts_with("__") { return None; }
    
    Some(sentry::init((dsn, sentry::ClientOptions {
        release: Some(format!("talktype@{}", env!("CARGO_PKG_VERSION")).into()),
        environment: Some("production".into()),
        send_default_pii: false,
        ..Default::default()
    })))
}
```

### **重要：Telemetry opt-in**

對 SayIt 改進：**預設 OFF**。Settings 頁面有 toggle，使用者明確開啟才送。

```rust
// 啟動時檢查 settings.telemetry_enabled
if !settings.telemetry_enabled {
    return None; // 不 init Sentry
}
```

## 隱私 / Telemetry policy

### 送的資料

- ✅ Rust panic / crash backtrace
- ✅ Frontend uncaught exception
- ✅ E2E latency metrics（透過 Sentry transactions）
- ✅ App version、OS、locale

### 絕不送

- ❌ Audio file
- ❌ Transcribed text
- ❌ User-set vocabulary（可能有個資）
- ❌ API keys
- ❌ Window title（target paste app 名稱）
- ❌ Settings 內容
- ❌ History records

### 隱私聲明

`PRIVACY.md` 在 repo root，README 連結。

## GitHub Issues / PR Templates

`.github/ISSUE_TEMPLATE/`:

- `bug_report.md` — 結構化欄位（OS、TalkType version、steps to reproduce、expected/actual）
- `feature_request.md`
- `question.md`

`.github/PULL_REQUEST_TEMPLATE.md`:

- Summary、Test plan、Screenshots（如 UI 變更）

## CONTRIBUTING.md / CODE_OF_CONDUCT.md

- `CONTRIBUTING.md` — 開發環境設定、PR 流程、commit message convention（conventional commits）
- `CODE_OF_CONDUCT.md` — Contributor Covenant 2.1

## 完整 GitHub Secrets 列表

Phase 2 完成時 GitHub repo Settings → Secrets and variables → Actions：

| Secret | 用途 | 何時設 |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Updater 簽 | 設 updater 時 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 同 | 同 |
| `APPLE_CERTIFICATE` | macOS Developer ID `.p12` Base64 | macOS 簽名前 |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` 密碼 | 同 |
| `APPLE_SIGNING_IDENTITY` | Identity 字串（如 `Developer ID Application: Your Name (TEAMID)`） | 同 |
| `APPLE_ID` | Apple ID email | Notarization 前 |
| `APPLE_PASSWORD` | App-Specific Password | 同 |
| `APPLE_TEAM_ID` | Apple Developer Team ID | 同 |
| `SENTRY_DSN` | Rust Sentry | Sentry 設好後 |
| `VITE_SENTRY_DSN` | Frontend Sentry | 同 |
| `SENTRY_AUTH_TOKEN` | Sourcemap upload | 同 |
| `SENTRY_ORG` | Sentry org slug | 同 |
| `SENTRY_PROJECT` | Sentry project slug | 同 |

## Phase 2 Checklist

完整 Phase 2 release ready 時：

- [ ] `scripts/release.sh` 寫好 + tested
- [ ] `release.yml` 寫好 + tested with workflow_dispatch
- [ ] Tauri updater pubkey embed in `tauri.conf.json`
- [ ] Updater private key 備份在 1Password
- [ ] Apple Developer Program 啟用、Cert 在 GitHub Secrets
- [ ] `Entitlements.plist` + `Info.plist` 寫好
- [ ] Sentry project 建好、DSN 在 Secrets
- [ ] Telemetry opt-in toggle 在 Settings
- [ ] PRIVACY.md 寫好、README 連結
- [ ] First public release tagged + auto-published
- [ ] `releases/latest/download/TalkType-*.{exe,dmg}` 三個檔案 resolve 到正確 binary
- [ ] Auto-updater 從 v0.X.0 升到 v0.X+1.0 work（手動 test）
- [ ] macOS notarization 通過（`spctl --assess --type execute --verbose TalkType.app`）
- [ ] Windows SmartScreen 警告處理（自簽 → 「仍要執行」/ SignPath 簽 → 無警告）

## 對 SayIt CI/release 的差異

| 維度 | SayIt | TalkType |
|---|---|---|
| Sentry release name | `sayit@<version>` | `talktype@<version>` |
| Telemetry | 預設 ON（implicit） | **預設 OFF + opt-in** |
| Lint in CI | 沒 ESLint | 有 ESLint + cargo clippy |
| Test in CI | 沒 cargo test | 有 cargo test |
| BMad-method | 重度使用 | 不用（複雜度太高） |
| Issue templates | 無 | 有（bug/feature/question） |
| Privacy policy | 無 | PRIVACY.md（必要） |

## 連結

- 架構 → [`01-architecture.md`](01-architecture.md)
- Phase 1 與 Phase 2 範圍對比 → [`../goals/02-phase2-release.md`](../goals/02-phase2-release.md)
- SayIt CI/release reference → [`../reference/sayit-cicd-analysis.md`](../reference/sayit-cicd-analysis.md)
- SayIt 改進建議 → [`../reference/sayit-improvements.md`](../reference/sayit-improvements.md)
