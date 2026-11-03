# Embedding In Agent Products

Use this reference when bundling Computer Pilot into Codex, Tenon, OpenClaw,
or any other shell-capable Agent runtime.

## Required Architecture

```text
Agent -> Computer Pilot skill -> existing shell -> short-lived cu CLI
      -> private per-user Broker -> macOS frameworks
```

Expose only the skill and ordinary `cu` commands. Do not register
`computer.*` native tools and do not expose the Broker socket or request
schema. Do not add MCP, stdio JSON-RPC, a native SDK, or a runtime-specific
adapter.

## Host Responsibilities

1. Install the complete skill directory unchanged.
2. Provide a supported official `cu` binary: either preinstall it with the
   bundled `scripts/install-native.sh` (use `--asset-directory` to install
   from locally bundled release assets without network access), or let the
   skill preflight self-install it. Both paths converge on the same fixed
   layout — `<install-root>/bin/cu` is the stable realpath that upgrades
   replace atomically, so macOS TCC grants survive official upgrades. Do not
   install with sudo, do not copy a raw binary to an unmanaged path, and do
   not re-sign official artifacts.
3. Put `<install-root>/bin` **first** on the Agent shell `PATH`. macOS ships
   an unrelated `/usr/bin/cu` (UUCP dialer) that otherwise wins: it answers
   `cu --version` with `cu (Taylor UUCP) 1.07` and fails every desktop
   command. The installer reports `path_ready=false` plus `shadowed_by` and
   `install_bin_dir` when a plain `cu` does not reach Computer Pilot.
4. Assign one stable `COMPUTER_PILOT_CLIENT_KEY` per logical Agent.
5. Assign an absolute, execution-owned `COMPUTER_PILOT_OUTPUT_DIR` on each
   shell call. Reused launchers must inherit the current directory rather than
   capture an earlier call's output path. Concurrent shell calls need separate
   directories so a host can attribute and adopt each result independently.
6. Let the Agent use its existing shell execution and file/image reading tools.
7. Preserve stdout, stderr, exit status, and JSON fields.
8. Keep the private Broker internal to Computer Pilot.

The installer accepts `COMPUTER_PILOT_INSTALL_ROOT` and
`COMPUTER_PILOT_BIN_DIR`; preserve both when the host supplies them. Keep the
command-link directory separate from `<install-root>/bin`, which owns the real
binary. Skill scripts are distributed as non-executable text and invoked with
`sh`; acquiring the Skill never runs the installer. An absent or unverified CLI
is installed/verified at first use through the ordinary shell admission boundary.

Do not generate a second tool catalog from `cu --help`; the skill plus runtime
help is the public contract.

## Compatibility

Read `../compatibility.json` before selecting a bundled artifact. Match:

- supported platform and architecture;
- CLI version range (`cli.minimum_version` through `cli.version`;
  `cli.tested_version` is the exact release the skill was validated against);
- machine schema version;
- skill/plugin version;
- required public integration model.

The manifest's `installation` object is the single source for the pinned
release: repository, asset template, installer path, fixed install layout,
and the signing policy (`signing.requirement` is the codesign designated
requirement official binaries must satisfy; `required_status`
`developer-id-notarized` marks the production identity).

Reject unsupported Intel Macs explicitly. Do not silently use an older raw
binary with a newer skill.

## Files

Provide an output directory per shell execution with permissions appropriate
for the current user, and inherit it on every CLI invocation in that execution.
Preserve the host-supplied directory; use a task-owned fallback only when none
is supplied. Reused launchers must not retain a previous execution's path. The
private Broker transports the current request's directory to the worker; it
must not reuse another execution's or Agent's directory.

Read returned absolute paths with the host's normal file or vision feature.

## Acceptance

Test at least:

- two Agents with different client keys and isolated refs/commands;
- parallel reads and serialized same-target mutations;
- user UI changes producing `stale_observation` before action dispatch;
- request replay, conflict, cancellation, expiration, and `unknown_outcome`;
- independent execution output directories with no overwrite or symlink traversal;
- permission continuity across an official version upgrade (the fixed
  `<install-root>/bin/cu` realpath must not change and the atomic replace
  must preserve existing TCC grants for signed identities);
- an offline `--asset-directory` install from bundled release assets;
- self-installed and bundled copies following identical skill/shell/CLI flow.
