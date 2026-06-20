# roundtable

The convener for a daily creative loop: one command runs `the-lunch` → `vicious-circle` → `conning-tower` in order, critiques the day's artifacts, and publishes a column — failing loud the moment a link in the chain breaks.

## Why it exists

Three sub-systems each ship a clean CLI. `the-lunch` lays out the day's artifacts; `vicious-circle` critiques them and crowns the best line; `conning-tower` composes and syndicates a column. Each works alone, and that was the problem — running the loop meant running three commands by hand, in the right order, every day, and noticing when one quietly failed. `roundtable` is the missing wire. It convenes the table once, passes each stage's output to the next, and refuses to continue past a broken step instead of publishing half a session.

## Install

Requires a Rust toolchain (edition 2021, Rust 1.85+) and `cargo`.

```sh
cargo install --path .
```

Or run the installer, which builds the binary and wires the systemd user timers (noon session, Monday bind), the SessionStart hook, and `~/.claude/settings.json`. It is idempotent and supports `--dry-run`:

```sh
./install.sh --dry-run   # print every action, mutate nothing
./install.sh             # build, install, enable timers
```

## Quickstart

See the plan without touching disk:

```sh
roundtable session --dry-run --date 2026-06-15
```

Run the full chain for today:

```sh
roundtable session            # the-lunch → vicious-circle → conning-tower
roundtable session --with-games   # also debate the day's first artifact
```

On success it prints, for example:

```
3 artifact(s) critiqued, column composed and syndicated to columns
```

If no session ran for a date, `digest` says so rather than inventing one:

```
$ roundtable digest --date 2026-06-15
roundtable · no session run for 2026-06-15 — run: roundtable session
```

## Commands

| Command | What it does |
|---------|--------------|
| `session` | Convene the full daily chain: lunch, critique each artifact, compose and syndicate the column. `--with-games` appends a debate; `--dry-run` prints the plan without mutating. |
| `digest` | Surface a date's crowned bon mot and column headline. Offline, local-only. `--format json` for machine output; defaults to yesterday. |
| `bind` | Bind columns since the last issue into a new issue via `new-yorker` (`issue` then `cover`). Idempotent — skips if an issue already covers the period. |
| `games` | Debate an article against an opponent: `wordsmith`, `pedant`, or `contrarian`. `--rounds` sets the count (default 3). |
| `weekly` | Build a weekly digest across all sources for an ISO week. `--format` is `text`, `markdown`, or `json`. |

## How it works

`session` resolves each tool by name on `PATH`, or from `--bin-dir`, or from a per-tool environment override (`ROUNDTABLE_LUNCH_BIN`, `ROUNDTABLE_CIRCLE_BIN`, `ROUNDTABLE_TOWER_BIN`, `ROUNDTABLE_NEWYORKER_BIN`). A missing required tool fails the session non-zero with a message naming it. State follows the XDG convention: `the-lunch` writes `$XDG_STATE_HOME/the-lunch/<date>/table.json`, and roundtable records to `$XDG_DATA_HOME/roundtable/<date>/ledger.jsonl`. Recording is deduplicated per artifact per date, so re-running a day is safe.

## Where it fits

Part of the creative-writing fleet. `roundtable` orchestrates `the-lunch`, `vicious-circle`, and `conning-tower`, and hands off to `new-yorker` for binding columns into issues. Installing it disables the standalone `the-lunch.timer`, since the session now convenes the table itself.
