# Agent-neutral stdio host

This is a minimal reference host for `cu bridge --stdio`. It uses only the
Python standard library and demonstrates the process boundary any local Agent
runtime can implement. It is an example, not a required SDK or runtime
dependency.

Run the passive default call:

```bash
python3 examples/stdio-host/host.py \
  --cu ./target/release/cu
```

Request only the capabilities needed by the tools exposed to the Agent:

```bash
python3 examples/stdio-host/host.py \
  --cu ./target/release/cu \
  --capability desktop.observe \
  --tool computer.snapshot \
  --arguments '{"app":"Finder","limit":20}'
```

The important host responsibilities shown in `host.py` are:

- launch an exact executable with an argv vector and no shell;
- initialize before discovery or calls;
- register only tools returned by `tools/list`;
- preserve caller-owned Command IDs and deadlines;
- branch on stable `error.data.code` values;
- return command-level errors to the Agent without blindly retrying mutations;
- send `shutdown` and close stdin during cleanup.

For production, translate returned screenshots and files into the Agent
runtime's native content representation and apply the host's approval and
retention policies. Protocol 1.0 does not coordinate multiple bridge processes;
do not let independent hosts mutate the same desktop concurrently.

The complete contract is in
[`docs/universal-agent-integration.md`](../../docs/universal-agent-integration.md).
