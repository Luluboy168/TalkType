#!/usr/bin/env bash
# protect-config.sh — PreToolUse hook for Edit|Write|MultiEdit
#
# Purpose:
#   - Hard-block edits to lockfiles (pnpm-lock.yaml, Cargo.lock, package-lock.json,
#     yarn.lock). These files are tooling-managed and must not be hand-edited.
#   - Soft-warn on tauri.conf.json and Cargo.toml (still allow, but surface a
#     warning so Claude confirms the edit is necessary).
#
# Spec:
#   - Reads JSON from stdin via Claude Code hook protocol.
#   - Tool input field used: tool_input.file_path.
#   - Hard-block: emit JSON with hookSpecificOutput.permissionDecision="deny"
#     and exit 0 (per Claude Code hooks JSON schema; PreToolUse).
#   - Soft-warn: emit JSON with systemMessage and exit 0.
#   - Otherwise: exit 0 silently.
#
# Reference: https://code.claude.com/docs/en/hooks
#
# JSON parsing strategy:
#   - Prefer jq when available (robust).
#   - Fallback to grep/sed (Git Bash on Windows often lacks jq).

set -uo pipefail

# Read all of stdin into a variable.
input="$(cat)"

extract_file_path() {
  local payload="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$payload" | jq -r '.tool_input.file_path // empty' 2>/dev/null
    return
  fi
  # Fallback: tolerant grep/sed extraction. Looks for "file_path":"..." within
  # tool_input. Not bullet-proof against unusual JSON (escaped quotes inside the
  # path) but adequate for typical absolute/relative paths Claude Code emits.
  printf '%s' "$payload" \
    | tr -d '\n' \
    | sed -n 's/.*"tool_input"[[:space:]]*:[[:space:]]*{[^}]*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'
}

file_path="$(extract_file_path "$input")"

# If we cannot determine a file_path, allow silently. We must not block on
# parsing-only failures.
if [ -z "$file_path" ]; then
  exit 0
fi

# Normalize path separators so Windows backslashes don't trip the matcher.
normalized="${file_path//\\//}"
basename="${normalized##*/}"

# --- Hard block: lockfiles ------------------------------------------------
case "$basename" in
  pnpm-lock.yaml|Cargo.lock|package-lock.json|yarn.lock)
    cat <<'JSON'
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "Lockfiles must not be hand-edited. Run 'pnpm install' (pnpm-lock.yaml) or 'cargo check' / 'cargo update' (Cargo.lock) to regenerate."
  }
}
JSON
    exit 0
    ;;
esac

# --- Soft warn: build / Tauri config -------------------------------------
case "$basename" in
  tauri.conf.json|Cargo.toml)
    cat <<JSON
{
  "systemMessage": "WARNING: Editing $basename — confirm the change is necessary (build / bundle config; impacts Tauri identifier, version, or capabilities)."
}
JSON
    exit 0
    ;;
esac

# Default: allow silently.
exit 0
