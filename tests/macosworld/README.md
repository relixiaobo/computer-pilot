# macOSWorld Integration — System Test for computer-pilot

Uses the [macOSWorld benchmark](https://github.com/showlab/macosworld) (202 tasks, 30 apps, 5 languages) as the standard system test.

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
git clone <computer-pilot-repo>
cd computer-pilot && cargo build --release
sudo cp target/release/cu /usr/local/bin/
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

For quick local testing without AWS EC2, run tasks manually:

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

## Expected Results

| Agent | macOSWorld Score (reported) |
|---|---|
| Claude Computer Use | >30% |
| GPT-4o | >30% |
| Open-source models | <5% |
| **cu + Claude Opus** | TBD |

Our advantage: AX tree provides structured element data (refs, roles, positions) instead of relying solely on screenshots, which should improve accuracy and reduce token costs.

## Key Differences from Screenshot-Based Agents

| Aspect | Screenshot agents | cu agent |
|---|---|---|
| Observation | Screenshot image (~1400 tokens) | AX tree text (~50 tokens/element) |
| Element targeting | Coordinate guessing from image | Exact ref-based click |
| Reliability | Coordinate drift on multi-step | Stable per-action re-snapshot |
| Fallback | None | OCR → Screenshot → Vision |
