#!/usr/bin/env bash
# rustfmt.sh — PostToolUse hook for Edit|Write|MultiEdit
#
# Purpose:
#   - Auto-format Rust source files in-place via `rustfmt` after edits.
#   - Non-blocking: never fails the tool call. If formatting fails, surface a
#     short note but exit 0 so Claude continues.
#
# Trigger filter (script-side):
#   - Only fires for *.rs files inside the workspace.
#   - Skips paths under target/ (build artifacts) or .git/.
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

case "$normalized" in
  *.rs) ;;
  *) exit 0 ;;
esac

case "$normalized" in
  */target/*|*/.git/*) exit 0 ;;
  target/*|.git/*) exit 0 ;;
esac

# `rustfmt` is part of the `rustfmt` rustup component. Our rust-toolchain.toml
# pins `components = ["rustfmt", "clippy"]` so this should always be available
# in the developer's Rust toolchain. If it's not, we silently skip rather than
# halting Claude's flow.
if ! command -v rustfmt >/dev/null 2>&1; then
  exit 0
fi

# Format in-place. We pipe stderr to /dev/null because rustfmt's diagnostics
# are not actionable here — non-blocking by design.
rustfmt --edition 2021 "$file_path" >/dev/null 2>&1 || true

exit 0
