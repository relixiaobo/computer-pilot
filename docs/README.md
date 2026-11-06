# Computer Pilot — Docs

Start with [release status and remaining work](ROADMAP.md). The latest recorded
release is **[v0.9.6](https://github.com/relixiaobo/computer-pilot/releases/tag/v0.9.6)**,
published with Developer ID signing and Apple notarization, and promoted to
`skill-stable` on 2026-11-07 (Asia/Shanghai).
For a new task, use the current working agreement and CLI reference below;
dated research and benchmark reports preserve evidence from their own runs.

## Quick Start

```bash
cargo build --release
./target/release/cu setup
./target/release/cu --human state com.apple.finder
./target/release/cu --human launch TextEdit
./target/release/cu --human type "hello" --app com.apple.TextEdit
./target/release/cu --human key cmd+s --app com.apple.TextEdit
./target/release/cu --human screenshot "Google Chrome"
```

## Automation Commands (31)

| Category | Commands |
|---|---|
| **Discover** | `setup`, `apps`, `menu`, `sdef`, `examples` |
| **Observe** | `state`, `snapshot`, `find`, `nearest`, `observe-region`, `screenshot`, `ocr`, `wait` |
| **Act** | `click`, `type`, `key`, `set-value`, `perform`, `scroll`, `hover`, `drag` |
| **Script & System** | `tell`, `defaults`, `window`, `launch`, `warm`, `why` |
| **Recover** | `status`, `commands`, `command`, `cancel` |

Agent hosts use these same commands through their existing shell. Computer
Pilot does not expose a separate bridge, MCP server, SDK, or host adapter.

Application display names are localized. Use the bundle identifier or the
`pid:<PID>` selector returned by `cu apps` for repeatable targeting. `launch`
also accepts the installed app name and waits using its resolved bundle identity.

Headline calls:
- **`cu state <app>`** — snapshot + windows + screenshot + frontmost when visual context is needed; use `cu tell` for scriptable data or `cu --compact snapshot <app>` for refs
- **`cu click <ref> --app X`** — AX action chain → CGEvent fallback; returns `verify_diff` / `verify_advice`, with generic `verified:null` and a separate scoped input-focus check
- **`cu type "..." --app X`** — Unicode CGEvent (no clipboard); auto-routes via paste for Chromium inputs / known chat apps (`paste_reason`)
- **`cu tell <app> '<AppleScript>'`** — direct data access for scriptable apps (`S` flag in `cu apps`)
- **`cu launch <name|bundleId>`** — spawn + wait for first AX-ready window
- **`cu why <ref> --app X`** — diagnose why a click/perform/set-value didn't take

Per-flag reference: run `cu <command> --help`, or read [`plugin/skills/computer-pilot/references/commands.md`](../plugin/skills/computer-pilot/references/commands.md).

## Documents

| Document | Purpose |
|----------|---------|
| [AGENTS.md](../AGENTS.md) / [CLAUDE.md](../CLAUDE.md) | Identical working agreement: design, release and testing rules |
| [ROADMAP.md](ROADMAP.md) | Release checkpoint, remaining work and clearly marked historical progress |
| [universal-agent-integration.md](universal-agent-integration.md) | CLI-only Agent integration and private Broker boundary |
| [CLI reference](../plugin/skills/computer-pilot/references/commands.md) | Current commands and output semantics; `cu <command> --help` is the runtime reference |
| [Agent tests](../tests/agent/README.md) | Current test protocol and index of dated results, including known failures |
| [Comparison matrix](../tests/agent/comparison-matrix.md) | Benchmark design and coverage still to be completed |
| [virtual-cursor-research.md](virtual-cursor-research.md) | Shipped implementation boundary and pinned upstream provenance |
| [competitive-analysis.md](competitive-analysis.md) | Feature comparison snapshot (frozen 2026-04-03 — see ROADMAP for current state) |
| [archive/](archive/) | Research, superseded plans, inventories, and candidate patches (frozen) |
