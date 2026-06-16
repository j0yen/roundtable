#!/usr/bin/env bash
# install.sh — build + install `roundtable` and wire the SessionStart hook.
#
# Idempotent: re-running does not duplicate settings.json entries.
# Requires: cargo, jq.

set -euo pipefail

SCRIPT_PATH="${BASH_SOURCE[0]:-$0}"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"

# ---------------------------------------------------------------------------
# Build + install the binary
# ---------------------------------------------------------------------------
command -v cargo >/dev/null 2>&1 || { echo "fatal: cargo not found"; exit 1; }

echo "→ building + installing roundtable via cargo install..."
cargo install --path "${SCRIPT_DIR}" --locked

if ! command -v roundtable >/dev/null 2>&1; then
    echo "! roundtable installed but not on PATH. Add ~/.cargo/bin to PATH:"
    echo "    export PATH=\"\$HOME/.cargo/bin:\$PATH\""
fi

echo "✓ roundtable binary installed."

# ---------------------------------------------------------------------------
# Install the SessionStart hook
# ---------------------------------------------------------------------------
HOOK_SRC="${SCRIPT_DIR}/hooks/roundtable-sessionstart.sh"
HOOK_DEST="${HOME}/.local/bin/roundtable-sessionstart"

if [ -f "${HOOK_SRC}" ]; then
    echo "→ installing SessionStart hook to ${HOOK_DEST}..."
    mkdir -p "$(dirname "${HOOK_DEST}")"
    cp -f "${HOOK_SRC}" "${HOOK_DEST}"
    chmod +x "${HOOK_DEST}"
    echo "✓ Hook installed at ${HOOK_DEST}"
else
    echo "! hooks/roundtable-sessionstart.sh not found — skipping hook install"
    exit 1
fi

# ---------------------------------------------------------------------------
# Wire into ~/.claude/settings.json (jq + atomic rename, idempotent)
# ---------------------------------------------------------------------------
SETTINGS="${HOME}/.claude/settings.json"
command -v jq >/dev/null 2>&1 || {
    echo "! jq not found — skipping settings.json wiring"
    echo "  Add manually:"
    echo "    {\"type\":\"command\",\"command\":\"${HOOK_DEST}\"}"
    echo "  to the first SessionStart hooks array in ${SETTINGS}"
    exit 0
}

if [ ! -f "${SETTINGS}" ]; then
    echo "! ${SETTINGS} not found — skipping settings.json wiring"
    exit 0
fi

# Check idempotency: is the hook already wired?
if jq -e --arg cmd "${HOOK_DEST}" \
    '.hooks.SessionStart[0].hooks[] | select(.command == $cmd)' \
    "${SETTINGS}" >/dev/null 2>&1; then
    echo "✓ SessionStart hook already wired in ${SETTINGS} — no change needed."
    exit 0
fi

# Snapshot before modification
TS="$(date +%s)"
cp "${SETTINGS}" "${SETTINGS}.bak.${TS}"
echo "→ snapshotted ${SETTINGS} to ${SETTINGS}.bak.${TS}"

# Atomic update: append the hook into the first SessionStart group's hooks array
TMPFILE="$(mktemp "${SETTINGS}.tmp.XXXXXX")"
jq --arg cmd "${HOOK_DEST}" \
    '.hooks.SessionStart[0].hooks += [{"type":"command","command":$cmd}]' \
    "${SETTINGS}" > "${TMPFILE}"
mv "${TMPFILE}" "${SETTINGS}"

echo "✓ Wired roundtable-sessionstart hook into ${SETTINGS}"
echo "  Entry added: {\"type\":\"command\",\"command\":\"${HOOK_DEST}\"}"
