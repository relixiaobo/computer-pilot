# macOSWorld Integration — System Test for computer-pilot

Uses the [macOSWorld benchmark](https://github.com/showlab/macosworld) (202 tasks, 30 apps, 5 languages) as the standard system test.

Status as of **2026-11-07**: a fixed local baseline sample is published below;
the full 133-task baseline and automated regression dashboard remain deferred
([Roadmap E1/E2](../../docs/ROADMAP.md#sprint-3--shipped-capabilities-and-deferred-observability)).
This is an optional L3/manual workflow, not a completed v0.9.3 release gate.

## How It Works

```
macOSWorld Framework        cu Agent Adapter          macOS EC2
┌──────────────┐     ┌────────────────────┐     ┌──────────────┐
│ Task loader  │────▶│ cu_agent.py        │────▶│ cu binary    │
│ Evaluator    │     │  observe: cu snapshot│    │ AX tree      │
│ State reset  │     │  act: cu click/key  │    │ CGEvent      │
│ Grading      │     │  LLM: Claude/GPT   │    │ Vision OCR   │
└──────────────┘     └────────────────────┘     └──────────────┘
```

## Setup

### 1. Clone macOSWorld

```bash
git clone https://github.com/showlab/macosworld.git
cd macosworld
pip install vncdotool boto3 sshtunnel httpx[socks] openai anthropic
```

### 2. Copy cu agent

```bash
cp /path/to/computer-pilot/tests/macosworld/cu_agent.py macosworld/agent/
```

### 3. Register agent

Add to `macosworld/agent/get_gui_agent.py`:

```python
elif "cu/" in gui_agent_name:
    from agent.cu_agent import CuAgent
    model = gui_agent_name.split("cu/")[1]
    return CuAgent(
        remote_client=remote_client,
        model=model,
    )
```

### 4. Install cu on macOS EC2

SSH into the EC2 instance and build cu:

```bash
ssh -i credential.pem ec2-user@<host>
# On the EC2 instance:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/relixiaobo/computer-pilot.git
cd computer-pilot && cargo build --release
install_root="${XDG_DATA_HOME:-$HOME/.local/share}/computer-pilot"
mkdir -p "$install_root/bin"
cp target/release/cu "$install_root/bin/.cu.new"
mv "$install_root/bin/.cu.new" "$install_root/bin/cu"
export PATH="$install_root/bin:$PATH"
cu setup  # Grant Accessibility + Screen Recording
```

### 5. Run benchmark

```bash
# Run English tasks on sys_apps category
python run.py \
  --gui_agent_name "cu/claude-opus-4-6" \
  --ssh_host <ec2-host> \
  --ssh_pkey credential.pem \
  --instance_id <instance-id> \
  --paths_to_eval_tasks tasks/sys_apps \
  --languages en \
  --max-steps 15

# Run all categories
python run.py \
  --gui_agent_name "cu/claude-opus-4-6" \
  --ssh_host <ec2-host> \
  --ssh_pkey credential.pem \
  --instance_id <instance-id> \
  --paths_to_eval_tasks tasks/sys_apps tasks/productivity tasks/media tasks/file_management tasks/sys_and_interface tasks/multi_apps tasks/advanced tasks/safety \
  --languages en \
  --max-steps 15
```

### 6. Evaluate results

```bash
python testbench.py --base_save_dir ./results
```

## Local Testing (No AWS)

The repository's `local_test_set.json` classifies 133 locally runnable tasks.
Use `run_selected.py --help` for the local runner. For a manual spot check:

```bash
# 1. Read a task
cat tasks/sys_apps/48cf0af3-0612-dbcd-14da-d5202eed6ce9.json | jq .task.en
# "Add Ong KC to contact with mobile number 96910380."

# 2. Use cu to accomplish it
cu apps
cu snapshot Contacts --limit 30
cu click 5 --app Contacts
cu type "Ong KC" --app Contacts
# ... etc

# 3. Verify with the grading command
osascript -e 'tell application "Contacts" to get the value of phones of (first person whose name is "Ong KC")' 2>/dev/null | grep -q "96910380" && echo "PASS" || echo "FAIL"
```

## Results status

The first tracked local baseline uses upstream macOSWorld revision
`e2ca8334b3765537e5c1c428ce57990248a1bb5f`, English tasks, the `safe_low`
`sys_apps` cohort, and a 12-step budget. It contains three fixed tasks chosen
for temporary or read-only UI state. The relay-supported `gpt-6.1-sol` run
passed **1/3** tasks; Calculator passed, Chess failed, and Dictionary reached
the step budget without completing. The unsupported `gpt-6.1-terra` attempt
made no model request and is excluded.

| Date | Model | Cohort | Tasks | Passed | Binary |
|---|---|---|---:|---:|---|
| 2026-11-07 | `gpt-6.1-sol` | safe_low / sys_apps | 3 | 1 | `cu 0.9.6` |

The sanitized result is [tracked here](../../tests/macosworld/results/2026-11-07-gpt-6.1-sol-safe-low-sys-apps.json).
Reproduce it with [`scripts/run-macosworld-baseline.sh`](../../scripts/run-macosworld-baseline.sh)
after checking out the pinned upstream revision:

```bash
git clone https://github.com/showlab/macosworld.git /tmp/macosworld
git -C /tmp/macosworld checkout e2ca8334b3765537e5c1c428ce57990248a1bb5f
MACOSWORLD_ROOT=/tmp/macosworld MODEL=gpt-6.1-sol \
  bash scripts/run-macosworld-baseline.sh
```

The [real-app pilot reports](../agent/README.md#status-and-evidence-index) use
a different protocol and must not be presented as macOSWorld scores.

## Key Differences from Screenshot-Based Agents

| Aspect | Screenshot agents | cu agent |
|---|---|---|
| Observation | Screenshot image (~1400 tokens) | AX tree text (~50 tokens/element) |
| Element targeting | Coordinate guessing from image | Exact ref-based click |
| Reliability | Coordinate drift on multi-step | Stable per-action re-snapshot |
| Fallback | None | OCR → Screenshot → Vision |
