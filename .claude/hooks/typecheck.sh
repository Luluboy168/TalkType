#!/usr/bin/env bash
# typecheck.sh — PostToolUse hook for Edit|Write|MultiEdit
#
# Purpose:
#   - Run `vue-tsc --noEmit` after a TypeScript or Vue file is edited.
#   - Non-blocking: report errors via systemMessage so Claude can self-correct
#     on the next turn, but never block the tool call.
#
# Trigger filter (script-side):
#   - Only fires for files ending in .ts, .tsx, .vue, or .mts.
#   - Skips test/spec/d.ts files (matches SayIt's pattern — type errors in
#     real source files matter more than in test fixtures, and tests get
#     covered by `pnpm test` / vitest separately).
#   - Skips node_modules / dist / target paths.
#
# Reference: https://code.claude.com/docs/en/hooks

# NOTE: We deliberately do NOT use `set -e` because we must report vue-tsc's
# failure via JSON output rather than letting the script exit non-zero.
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

# Always exit 0 silently when no file path or wrong extension — non-blocking by design.
[ -z "$file_path" ] && exit 0

normalized="${file_path//\\//}"

# Skip non-TS/Vue files.
case "$normalized" in
  *.ts|*.tsx|*.vue|*.mts) ;;
  *) exit 0 ;;
esac

# Skip tests, type definition files, and generated/vendored paths.
# Patterns cover both absolute paths (*/foo/*) and relative ones (foo/*).
case "$normalized" in
  *.test.ts|*.test.tsx|*.spec.ts|*.spec.tsx|*.d.ts) exit 0 ;;
  */node_modules/*|*/dist/*|*/target/*|*/.git/*) exit 0 ;;
  node_modules/*|dist/*|target/*|.git/*) exit 0 ;;
esac

# Resolve project root via $CLAUDE_PROJECT_DIR (preferred — Claude Code sets
# this env var) or fall back to the script's parent directory.
project_root="${CLAUDE_PROJECT_DIR:-}"
if [ -z "$project_root" ]; then
  project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fi

cd "$project_root" || exit 0

# Run vue-tsc. Capture combined output; tsc reports diagnostics on stdout.
# We don't pre-flight `command -v` because we want a graceful warning if
# devDependencies are not yet installed (rather than silently doing nothing).
output="$(npx --no-install vue-tsc --noEmit 2>&1)"
status=$?

if [ $status -eq 0 ]; then
  exit 0
fi

# Trim output to keep the systemMessage manageable (first 30 lines).
trimmed="$(printf '%s\n' "$output" | head -n 30)"

# Emit JSON via a heredoc with python so we get correct JSON escaping for
# multi-line / special-char content. Falls back to a best-effort manual escape
# if python is not available.
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
  # Manual fallback: escape backslashes and double-quotes, replace newlines.
  escaped="$(printf '%s' "$message" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' | tr '\n' '\f' | sed 's/\f/\\n/g')"
  printf '{"systemMessage":"%s"}\n' "$escaped"
}

emit_json "vue-tsc reported errors after editing $file_path:
$trimmed"

# Exit 0 — we only want to report, not block.
exit 0
