#!/usr/bin/env bash
# eslint.sh — PostToolUse hook for Edit|Write|MultiEdit
#
# Purpose:
#   - Auto-fix ESLint issues on TS/JS/Vue files via `eslint --fix`.
#   - Non-blocking: report (without blocking) any lint failures that --fix
#     could not resolve, so Claude can address them on the next turn.
#
# Trigger filter (script-side):
#   - Only fires for files ending in .ts, .tsx, .js, .jsx, .mjs, .cjs, .vue.
#   - Skips src/components/ui/** — these are shadcn-vue generated and listed
#     in eslint.config.js ignores already; double-skipping here also avoids
#     spurious npx invocations.
#   - Skips node_modules / dist / target / .git paths.
#
# Reference: https://code.claude.com/docs/en/hooks

set -u

input="$(cat)"

extract_file_path() {
  local payload="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$payload" | jq -r '.tool_input.file_path // empty' 2>/dev/null
    return
  fi
  printf '%s' "$payload" \
    | tr -d '\n' \
    | sed -n 's/.*"tool_input"[[:space:]]*:[[:space:]]*{[^}]*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'
}

file_path="$(extract_file_path "$input")"

[ -z "$file_path" ] && exit 0

normalized="${file_path//\\//}"

# Lintable extensions only.
case "$normalized" in
  *.ts|*.tsx|*.js|*.jsx|*.mjs|*.cjs|*.vue) ;;
  *) exit 0 ;;
esac

# Skip vendored / generated paths.
# Patterns intentionally cover both absolute paths (matched via */foo/*) and
# relative paths emitted by tools without leading components (foo/*).
case "$normalized" in
  */node_modules/*|*/dist/*|*/target/*|*/.git/*) exit 0 ;;
  node_modules/*|dist/*|target/*|.git/*) exit 0 ;;
  */src/components/ui/*) exit 0 ;;
  src/components/ui/*) exit 0 ;;
esac

# Resolve project root from $CLAUDE_PROJECT_DIR or walk up from the script.
project_root="${CLAUDE_PROJECT_DIR:-}"
if [ -z "$project_root" ]; then
  project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fi

cd "$project_root" || exit 0

# Run eslint --fix. Use --no-install to avoid pulling a fresh copy when
# devDependencies are missing — we'd rather skip silently than hang.
output="$(npx --no-install eslint --fix "$file_path" 2>&1)"
status=$?

if [ $status -eq 0 ]; then
  exit 0
fi

# Truncate to first 30 lines for the systemMessage payload.
trimmed="$(printf '%s\n' "$output" | head -n 30)"

emit_json() {
  local message="$1"
  if command -v python >/dev/null 2>&1; then
    python - <<PY
import json, sys
print(json.dumps({"systemMessage": $(printf '%s' "$message" | python -c 'import json,sys; print(json.dumps(sys.stdin.read()))')}))
PY
    return
  fi
  if command -v python3 >/dev/null 2>&1; then
    python3 - <<PY
import json, sys
print(json.dumps({"systemMessage": $(printf '%s' "$message" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')}))
PY
    return
  fi
  escaped="$(printf '%s' "$message" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' | tr '\n' '\f' | sed 's/\f/\\n/g')"
  printf '{"systemMessage":"%s"}\n' "$escaped"
}

emit_json "eslint --fix reported remaining issues in $file_path (non-blocking):
$trimmed"

exit 0
