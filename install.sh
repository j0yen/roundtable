#!/usr/bin/env bash
# install.sh — build + install `roundtable`, wire systemd timers, and disable
# the now-subsumed the-lunch.timer.
#
# Idempotent: re-running does not duplicate settings.json entries or unit files.
# Supports --dry-run: prints every action without mutating anything.
# Requires: cargo, jq.

set -euo pipefail

SCRIPT_PATH="${BASH_SOURCE[0]:-$0}"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"

DRY_RUN=0
for arg in "$@"; do
    case "$arg" in
        --dry-run|--check) DRY_RUN=1 ;;
    esac
done

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
run() {
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "[dry-run] $*"
    else
        "$@"
    fi
}

run_quiet() {
    # Like run but suppresses stdout for cleanliness
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "[dry-run] $*"
    else
        "$@" >/dev/null
    fi
}

# ---------------------------------------------------------------------------
# Build + install the binary
# ---------------------------------------------------------------------------
command -v cargo >/dev/null 2>&1 || { echo "fatal: cargo not found"; exit 1; }

echo "→ building + installing roundtable via cargo install..."
run cargo install --path "${SCRIPT_DIR}" --locked

if [ "$DRY_RUN" -eq 0 ] && ! command -v roundtable >/dev/null 2>&1; then
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
    run mkdir -p "$(dirname "${HOOK_DEST}")"
    run cp -f "${HOOK_SRC}" "${HOOK_DEST}"
    run chmod +x "${HOOK_DEST}"
    echo "✓ Hook installed at ${HOOK_DEST}"
else
    echo "! hooks/roundtable-sessionstart.sh not found — skipping hook install"
    [ "$DRY_RUN" -eq 1 ] || exit 1
fi

# ---------------------------------------------------------------------------
# Install systemd user units (idempotent)
# ---------------------------------------------------------------------------
UNITS_SRC="${SCRIPT_DIR}/units"
UNITS_DEST="${HOME}/.config/systemd/user"

UNIT_FILES=(
    roundtable.service
    roundtable.timer
    roundtable-bind.service
    roundtable-bind.timer
)

echo "→ installing systemd user units to ${UNITS_DEST}..."
run mkdir -p "${UNITS_DEST}"

for unit in "${UNIT_FILES[@]}"; do
    src="${UNITS_SRC}/${unit}"
    dest="${UNITS_DEST}/${unit}"
    if [ ! -f "${src}" ]; then
        echo "! missing unit file: ${src}"
        exit 1
    fi
    if [ "$DRY_RUN" -eq 0 ] && [ -f "${dest}" ] && cmp -s "${src}" "${dest}"; then
        echo "  ✓ ${unit} already up-to-date"
    else
        echo "  → cp ${src} → ${dest}"
        run cp -f "${src}" "${dest}"
    fi
done

run systemctl --user daemon-reload
echo "✓ daemon-reload done."

# Enable + start timers (idempotent: --now is a no-op if already active)
echo "→ enabling + starting roundtable timers..."
run systemctl --user enable --now roundtable.timer roundtable-bind.timer
echo "✓ roundtable.timer and roundtable-bind.timer enabled."

# ---------------------------------------------------------------------------
# Disable the subsumed the-lunch.timer (AC5)
# ---------------------------------------------------------------------------
if [ "$DRY_RUN" -eq 1 ]; then
    echo "[dry-run] systemctl --user is-enabled the-lunch.timer  # check if enabled"
    echo "[dry-run] systemctl --user disable --now the-lunch.timer  # if enabled"
    echo "  (roundtable session now convenes the table; the-lunch.timer would fire twice)"
else
    if systemctl --user is-enabled the-lunch.timer >/dev/null 2>&1; then
        echo "→ disabling the-lunch.timer (roundtable session now convenes the table;"
        echo "  the lunch still happens, via the fuller roundtable chain)."
        systemctl --user disable --now the-lunch.timer || true
        echo "✓ the-lunch.timer disabled."
    else
        echo "  ✓ the-lunch.timer not enabled — nothing to disable."
    fi
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
else
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "[dry-run] would append {\"type\":\"command\",\"command\":\"${HOOK_DEST}\"} to .hooks.SessionStart[0].hooks in ${SETTINGS}"
    else
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
    fi
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "Done! Run manually:"
echo "  roundtable session --with-games   # full noon session with games"
echo "  roundtable bind                   # weekly bind"
echo ""
echo "Timers:"
if [ "$DRY_RUN" -eq 0 ]; then
    systemctl --user list-timers roundtable.timer roundtable-bind.timer 2>/dev/null || true
fi
