# Universal Agent Integration

Status: protocol 1.0 implemented

## Objective

Any local Agent product that can launch a process must be able to bundle an
exact `cu` executable and use the complete Computer Pilot command surface
without importing Rust code, parsing human output, invoking a shell, or
depending on an Agent-specific SDK.

Tenon, OpenClaw, Pi, Codex, and other runtimes are consumers and acceptance
fixtures only. Production code must not model their thread, turn, run, task, or
provider concepts.

## Decisions

- The executable protocol is the SDK.
- The existing one-shot CLI remains supported.
- Embedded hosts launch `cu bridge --stdio` directly with an absolute path.
- The bridge uses JSON-RPC 2.0 over newline-delimited UTF-8 JSON.
- Stdout is protocol-only. Diagnostics go to stderr.
- MCP and native language SDKs are not public Computer Pilot surfaces.
- The canonical tool manifest is generated from the same Clap definitions that
  validate one-shot CLI calls.
- A host owns user approval UX. Computer Pilot publishes capabilities,
  mutability, sensitivity, and routing evidence so the host can make that
  decision without guessing.
- The protocol is additive within one major version. Incompatible behavior
  requires a new major version.

## Protocol 1.0

Protocol 1.0 establishes a connection-scoped integration boundary. It is a
complete replacement for shell wrappers, but it does not claim cross-process
desktop ownership or protected Artifact storage.

Required methods:

```text
initialize
tools/list
tools/call
shutdown
```

Each line contains exactly one JSON-RPC message. Requests are processed in
order. The bridge rejects malformed framing, calls before initialization,
unknown methods, unknown tool arguments, unavailable capabilities, and results
that cannot be decoded as machine JSON.

`initialize` negotiates protocol `1.0`, records the caller's product and
instance identity, and intersects requested capabilities with the capabilities
allowed when the bridge was launched. A capability denied at launch cannot be
enabled later by a tool call.

`tools/list` returns only tools available under the negotiated capabilities.
Every definition contains:

- a stable `computer.*` name;
- the corresponding one-shot command;
- description and JSON input schema;
- generic machine-result schema;
- required capabilities;
- mutating and sensitivity metadata;
- possible file Artifact kinds.

`tools/call` accepts a tool name, structured arguments, an optional caller-owned
Command ID, and an optional deadline. The bridge converts structured arguments
to a strict argv vector and launches the same executable directly, never
through a shell. Reusing a Command ID with the same call returns the bounded
cached terminal result. Reusing it for a different call fails.

If a read-only child exceeds its deadline, the result is `command_expired`. If
a mutating child exceeds its deadline or loses its result after launch, the
result is `unknown_outcome`; hosts must observe current UI state before deciding
what to do and must never replay the mutation automatically.

## Wire Contract

Launch the bridge as an argv vector, not a shell string:

```text
["/absolute/path/to/cu", "bridge", "--stdio"]
```

Write one compact JSON request plus `\n`, then read one response line. Request
IDs must be strings or numbers. Protocol 1.0 is ordered and request/response
only; it does not emit notifications. The maximum input line is 4 MiB, the
maximum response is 16 MiB, and the maximum requested deadline is 300 seconds.

The first request initializes the connection:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"client":{"id":"com.example.agent","name":"Example Agent","version":"2.4.0","instanceId":"local-session-opaque"},"protocol":{"min":{"major":1,"minor":0},"max":{"major":1,"minor":0}},"requestedCapabilities":["desktop.discover","desktop.observe","desktop.input"],"launchMode":"embedded"}}
```

The result reports the selected protocol, executable version, opaque connection
ID, limits, and `granted`, launch-time `denied`, and unknown `unsupported`
capabilities. Do not call a tool that was not granted.

Discover the runtime manifest after initialization:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

Register only the returned definitions. Treat `inputSchema` as authoritative;
preserve `requiredCapabilities`, `mutating`, `idempotency`, `sensitivity`, and
`artifactKinds` in the host's own tool and approval model.

Invoke a tool with structured arguments:

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"computer.snapshot","arguments":{"app":"Finder","limit":50},"commandId":"host-command-42","deadlineMs":30000}}
```

A completed command is returned inside the JSON-RPC result:

```json
{"jsonrpc":"2.0","id":3,"result":{"command":{"id":"host-command-42","status":"completed","mutating":false},"result":{"schema_version":"1.0","ok":true}}}
```

An underlying `cu` failure is also a terminal JSON-RPC result so it can be
cached against the Command ID:

```json
{"jsonrpc":"2.0","id":3,"result":{"command":{"id":"host-command-42","status":"failed","mutating":false},"error":{"schema_version":"1.0","ok":false,"code":"command_failed","error":"bounded message","retryable":false}}}
```

Protocol/validation failures use the JSON-RPC error channel. Branch on
`error.data.code`, never the English message:

```json
{"jsonrpc":"2.0","id":3,"error":{"code":-32003,"message":"bounded message","data":{"code":"capability_denied","retryable":false,"context":{"capability":"desktop.input"}}}}
```

Stable protocol codes in 1.0 are `parse_error`, `invalid_argument`,
`protocol_incompatible`, `already_initialized`, `not_initialized`,
`method_not_found`, `tool_not_found`, `capability_denied`, `message_too_large`,
`result_too_large`, and `internal_error`. Terminal command errors additionally use
`command_expired` and `unknown_outcome`. `retryable:true` means a later call may
be valid; it never authorizes replaying a mutation.

Shut down explicitly when possible; closing stdin provides the same process
cleanup for a disconnected host:

```json
{"jsonrpc":"2.0","id":4,"method":"shutdown","params":{}}
```

A complete standard-library Python implementation of this lifecycle is in
[`examples/stdio-host/host.py`](../examples/stdio-host/host.py). It is an
executable reference and acceptance fixture, not a required language SDK.

## Machine Result Contract

Explicit `cu --json <command>` is the stable one-shot machine mode. Successful
object results include:

```json
{"schema_version":"1.0","ok":true}
```

Failures exit non-zero and include at least:

```json
{
  "schema_version":"1.0",
  "ok":false,
  "code":"command_failed",
  "error":"bounded message",
  "retryable":false
}
```

Existing fields such as `hint`, `suggested_next`, `diagnostics`, `method`,
`verified`, `verify_advice`, `confidence_hint`, and `screenshot_error` remain
part of the result. Adding `schema_version` and a stable error code does not
remove those recovery signals.

## Capabilities

Protocol 1.0 defines these launch-time capabilities:

```text
desktop.discover
desktop.observe
desktop.capture
desktop.input
desktop.pointer
desktop.window
desktop.app
desktop.script
desktop.defaults
```

Capabilities reduce exposure; they are not proof of user consent. In
particular, `desktop.script`, `desktop.defaults`, global input flags, window
closing, and focus-changing operations remain high-risk decisions for the host.

## Process and File Rules

- Hosts launch the exact executable directly, without `sh -c`, `bash -c`, or
  another shell.
- Hosts should resolve a bundled or project-pinned binary before consulting
  `PATH`.
- Hosts must bound stdin lines, stdout results, stderr, and tool deadlines.
- For screenshots and annotated observations in protocol 1.0, the host creates
  a private temporary directory, passes an absolute output path, ingests the
  result into its native image/payload model, and deletes the owned file.
- A host must not persist AX trees, OCR results, typed text, or screenshots
  unless its own retention policy explicitly allows it.

## Planned Protocol Increments

Protocol 1.1 will add cross-host coordination and protected media:

- Agent-neutral Workspaces and renewable ControlLeases;
- Observation IDs that scope refs to app, PID, window, and snapshot identity;
- read concurrency, per-target mutation serialization, and desktop-wide leases
  for global pointer/focus operations;
- protected Artifact descriptors with owner, mode `0600`, quota, TTL, export,
  and explicit release;
- Command query/cancel and bounded event replay;
- crash cleanup without persistent UI target state.

This increment requires changing the current no-daemon implementation boundary.
It must land only with a per-user ownership and cleanup design plus multi-host
acceptance tests. Protocol 1.0 deliberately reports no guarantee that two
independent bridge processes can safely mutate the same desktop concurrently.

## Acceptance

The black-box conformance runner must launch an exact command and verify, using
only stdin/stdout:

- initialization and version/capability negotiation;
- runtime tool discovery;
- a successful read-only tool call;
- stable invalid-tool, unknown-argument, and capability errors;
- duplicate and conflicting Command ID behavior;
- read-only deadline behavior;
- stdout protocol purity;
- graceful shutdown and EOF cleanup.

One-shot command tests continue to cover the underlying macOS behavior. Agent
E2E remains the release gate for whether a model can use the exposed semantics.
