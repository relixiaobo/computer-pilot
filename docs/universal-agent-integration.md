# Universal Agent Integration

Status: CLI-only public boundary and private per-user Broker implemented.

## Goal

Any shell-capable Agent can control macOS through the same Computer Pilot skill
and the same `cu` CLI, regardless of whether Computer Pilot was self-installed,
globally installed, or bundled by an Agent product.

```text
Agent -> Computer Pilot skill -> existing shell tool -> short-lived cu CLI
      -> private per-user Broker -> AX / CGEvent / SCK / Apple Events
```

Tenon, OpenClaw, Codex, and other runtimes are consumers and acceptance
fixtures only. Computer Pilot production code does not model their tool, turn,
thread, run, task, or provider concepts.

## Public Boundary

- The Computer Pilot skill teaches the Agent the operating workflow.
- The Agent uses its existing shell capability to invoke `cu`.
- `cu --help` and `cu <command> --help` are the runtime command inventory.
- JSON output is the stable programmatic CLI result.
- A product may place a pinned `cu` binary on the Agent command environment's
  `PATH`.
- A product assigns one stable `COMPUTER_PILOT_CLIENT_KEY` per logical Agent.
- File-producing tasks use an absolute, task-owned
  `COMPUTER_PILOT_OUTPUT_DIR`.

Computer Pilot does not expose `bridge --stdio`, JSON-RPC tools, MCP, a Native
SDK, or Agent-specific adapters. Hosts must not map the CLI commands into a
second native tool catalog. Public integration behavior must remain identical
across Agent products.

## Private Broker Boundary

The per-user Broker is an internal Computer Pilot implementation detail. It is
not an embedding API. Short-lived CLI processes use it to share the macOS TCC
identity and coordinate desktop state safely.

The Broker owns:

- stable client workspaces derived from the client key;
- bounded command records, request idempotency, deadlines, cancellation, and
  `unknown_outcome` recovery;
- Observation records binding refs to client, PID, bundle, window, and AX
  generation;
- mutation serialization per target application/resource and desktop-wide
  locking for global pointer, focus, and clipboard operations;
- bounded, expiring Observation state plus secure output-directory propagation;
- permission status and the fixed signed application identity used for TCC.

The Broker must be discoverable only by the local user, authenticate local
clients, bind its socket and state with user-only permissions, and negotiate
internal compatibility without exposing that transport to Agent hosts.

## Observation Safety

Every ref-producing observation returns an `observation_id`. A ref action is
resolved within the invoking client's Observation. Before dispatch, Computer
Pilot validates at least:

- client key;
- PID and bundle identifier;
- AX-derived window identity;
- AX generation and current target attributes.

If the app, window, generation, or target changed, the action fails with
`stale_observation`. It must never execute against a ref that merely occupies
the same sequential number in a newer tree. User activity invalidates state;
Computer Pilot does not try to lock the user out of the desktop.

## Command Recovery

The common CLI surface includes:

```text
cu status
cu commands
cu command <id>
cu cancel <id>
--client-key <key>
--request-id <id>
--timeout <milliseconds>
```

Environment equivalents:

```text
COMPUTER_PILOT_CLIENT_KEY=<stable logical Agent identity>
COMPUTER_PILOT_OUTPUT_DIR=<absolute task-owned directory>
```

Stable errors distinguish invalid input, permissions, missing apps/windows,
stale observations, busy targets, protected capture, failed verification,
cancellation, expiration, and uncertain mutation outcomes. Callers branch on
`code`, not English text. `retryable:true` never means a mutation is safe to
replay.

## File Results

Screenshots and other files return an absolute path, MIME type, byte size,
dimensions, and scale. Computer Pilot writes atomically, uses mode `0600`, does
not overwrite by default, and refuses symlink traversal. When no explicit path
is supplied, `COMPUTER_PILOT_OUTPUT_DIR` is the task-owned destination.

The CLI returns paths; Agents use their host's existing image and file tools to
read them. No transport-specific image result is required.

## Embedding Checklist

1. Bundle or install a supported Apple Silicon `cu` version.
2. Install the complete Computer Pilot skill without rewriting its commands.
3. Put the binary directory on the Agent shell environment's `PATH`.
4. Set a stable client key per logical Agent, not per command invocation.
5. Set an absolute output directory owned by the current task.
6. Let the Agent invoke ordinary `cu` commands through its existing shell.
7. Do not register native `computer.*` tools or depend on private Broker APIs.

## Acceptance

- Self-installed and product-bundled copies use the same skill and CLI path.
- Two Agents have isolated Observations, refs, commands, and output state.
- Mutations to the same window serialize; independent reads may proceed.
- User UI changes produce `stale_observation` before mutation dispatch.
- Stable request IDs prevent duplicate mutation dispatch during recovery.
- Broker upgrades preserve the fixed macOS permission identity.
- Signed/notarized Apple Silicon artifacts, checksums, plugin/skill archives,
  compatibility metadata, and a release index share one version.
