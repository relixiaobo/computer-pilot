# Computer Pilot — Docs

## Quick Start

```bash
cargo build --release
./target/release/cu setup
./target/release/cu --human state Finder
./target/release/cu --human launch TextEdit
./target/release/cu --human type "hello" --app TextEdit
./target/release/cu --human key cmd+s --app TextEdit
./target/release/cu --human screenshot "Google Chrome"
python3 scripts/run-stdio-conformance.py -- ./target/release/cu
```

## Automation Commands (27)

| Category | Commands |
|---|---|
| **Discover** | `setup`, `apps`, `menu`, `sdef`, `examples` |
| **Observe** | `state`, `snapshot`, `find`, `nearest`, `observe-region`, `screenshot`, `ocr`, `wait` |
| **Act** | `click`, `type`, `key`, `set-value`, `perform`, `scroll`, `hover`, `drag` |
| **Script & System** | `tell`, `defaults`, `window`, `launch`, `warm`, `why` |

Embedded hosts use the additional `bridge --stdio` integration command. See
the [reference Python host](../examples/stdio-host/README.md).

Headline calls:
- **`cu state <app>`** — canonical first call: snapshot + windows + screenshot + frontmost in one round-trip
- **`cu click <ref> --app X`** — 15-step AX action chain → CGEvent fallback; verify-on-by-default returns `verified` + `verify_advice`
- **`cu type "..." --app X`** — Unicode CGEvent (no clipboard); auto-routes via paste for CJK / chat apps (`paste_reason`)
- **`cu tell <app> '<AppleScript>'`** — direct data access for scriptable apps (`S` flag in `cu apps`)
- **`cu launch <name|bundleId>`** — spawn + wait for first AX-ready window
- **`cu why <ref> --app X`** — diagnose why a click/perform/set-value didn't take

Per-flag reference: run `cu <command> --help`, or read [`plugin/skills/computer-pilot/references/commands.md`](../plugin/skills/computer-pilot/references/commands.md).

## Documents

| Document | Purpose |
|----------|---------|
| [CLAUDE.md](../CLAUDE.md) | Design rules, agent reliability principles, testing rules |
| [ROADMAP.md](ROADMAP.md) | Sprint progress + reliability work — current source of truth |
| [universal-agent-integration.md](universal-agent-integration.md) | Agent-neutral stdio protocol, capability, and embedding contract |
| [examples/stdio-host](../examples/stdio-host/README.md) | Standard-library reference host for embedding `cu bridge --stdio` |
| [competitive-analysis.md](competitive-analysis.md) | Feature comparison snapshot (frozen 2026-04-03 — see ROADMAP for current state) |
| [archive/](archive/) | Research and historical design docs (frozen) |
