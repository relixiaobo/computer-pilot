"""
macOSWorld agent adapter for computer-pilot (cu).

Uses `cu` CLI over SSH instead of VNC screenshot+mouse.
Observation: cu snapshot (AX tree) + cu screenshot (fallback)
Actions: cu click, cu key, cu type, cu scroll, cu drag

Usage in macOSWorld:
  1. Copy this file to macosworld/agent/cu_agent.py
  2. Register in macosworld/agent/get_gui_agent.py
  3. Install cu binary on the macOS EC2 instance
  4. Run: python run.py --gui_agent_name cu/claude-opus-4-6 ...
"""

import json
import os
import time
import base64
from pathlib import Path
from utils.VNCClient import VNCClient_SSH
from utils.log import print_message

# Default LLM for decision-making. Override via --gui_agent_name cu/<model>
DEFAULT_MODEL = "claude-opus-4-6"

SYSTEM_PROMPT = """You are a macOS desktop automation agent. You control a Mac through the `cu` CLI tool.

## Available Commands (run via SSH)
- `cu apps` — List running apps (S = scriptable)
- `cu menu <app>` — List ALL menu bar items of any app (works for ALL apps)
- `cu sdef <app>` — Show scripting dictionary (classes, properties, commands)
- `cu tell <app> '<AppleScript>'` — Execute AppleScript (auto-wrapped in tell block)
- `cu defaults read/write <domain> [key] [value]` — Read/write macOS preferences
- `cu window list/move/resize/focus/minimize/close --app <name>` — Window management
- `cu snapshot [app] --limit N` — Get UI elements with [ref] numbers + window frame
- `cu screenshot [app] --path /tmp/shot.png` — Capture window screenshot
- `cu ocr [app]` — OCR text recognition
- `cu click <ref> --app <name>` — Click element by ref (AX action first, CGEvent fallback)
- `cu click <x> <y>` — Click by screen coordinates
- `cu click --text "label" --app <name>` — Click text via OCR (for AX-sparse UI)
- `cu click <ref> --right` / `--double-click` — Right-click / double-click
- `cu key <combo> --app <name>` — Keyboard shortcut (e.g., cmd+c, enter)
- `cu type <text> --app <name>` — Type text (clipboard paste, safe with any IME)
- `cu scroll <direction> <amount> --x X --y Y` — Scroll
- `cu drag <x1> <y1> <x2> <y2>` — Drag
- `cu wait --text <text> --app <name> --timeout N` — Wait for UI condition

## Strategy
1. Start with `cu apps` to see what's running. Apps marked `S` are scriptable.
2. **For scriptable apps**: prefer `cu sdef <app>` to discover commands, then `cu tell <app> '...'` to act. Scripting is faster and more reliable than UI automation.
3. **For non-scriptable apps**: use `cu menu <app>` to see what features exist, then `cu snapshot` to get clickable elements, then click/key/type.
4. **For system settings**: use `cu defaults write <domain> <key> <value>` instead of navigating System Settings UI.
5. **For window operations**: use `cu window` instead of dragging/clicking title bars.
6. If snapshot is sparse, try `cu ocr` or `cu click --text "label"`.

## Rules
- Always observe before acting. Run `cu snapshot` or `cu apps` first.
- Use `--app` to target specific apps and avoid focus issues.
- Refs are ephemeral — they change after every action. Always re-snapshot.
- When done, respond with DONE. If stuck, respond with FAIL.
- Think step by step. After each action, verify the result with a new snapshot.
"""


class CuAgent:
    """macOSWorld agent that uses `cu` CLI for observation and action."""

    def __init__(self, remote_client: VNCClient_SSH, model: str = DEFAULT_MODEL, **kwargs):
        self.remote_client = remote_client
        self.model = model
        self.messages = []
        self.total_input_tokens = 0
        self.total_output_tokens = 0

        # Detect which LLM SDK to use
        if "claude" in model or "opus" in model or "sonnet" in model:
            import anthropic
            self.llm_client = anthropic.Anthropic()
            self.llm_provider = "anthropic"
        elif "gpt" in model or "o1" in model or "o3" in model:
            import openai
            self.llm_client = openai.OpenAI()
            self.llm_provider = "openai"
        else:
            raise ValueError(f"Unknown model: {model}. Use claude-* or gpt-* prefix.")

    def cu(self, command: str) -> str:
        """Run a cu command on the remote macOS via SSH."""
        full_cmd = f"/usr/local/bin/cu {command}"
        try:
            result = self.remote_client.run_ssh_command(full_cmd)
            return result.strip() if result else ""
        except Exception as e:
            return json.dumps({"ok": False, "error": str(e)})

    def observe(self) -> str:
        """Get current UI state via cu snapshot."""
        # First get the app list to know what's running
        apps_json = self.cu("apps")

        # Snapshot the frontmost app
        snapshot_json = self.cu("snapshot --limit 50")

        return f"=== Running Apps ===\n{apps_json}\n\n=== UI Snapshot ===\n{snapshot_json}"

    def screenshot_b64(self) -> str:
        """Capture screenshot and return as base64 for vision models."""
        self.cu("screenshot --path /tmp/cu_step.png")
        try:
            # Read the file via SSH
            result = self.remote_client.run_ssh_command("base64 < /tmp/cu_step.png")
            return result.strip() if result else ""
        except Exception:
            return ""

    def call_llm(self, task: str, observation: str, screenshot_b64: str = None) -> str:
        """Ask the LLM to decide the next action."""
        user_content = f"Task: {task}\n\nCurrent observation:\n{observation}"

        if self.llm_provider == "anthropic":
            return self._call_anthropic(user_content, screenshot_b64)
        else:
            return self._call_openai(user_content, screenshot_b64)

    def _call_anthropic(self, user_content: str, screenshot_b64: str = None) -> str:
        import anthropic

        content = [{"type": "text", "text": user_content}]
        if screenshot_b64:
            content.append({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": screenshot_b64,
                }
            })

        self.messages.append({"role": "user", "content": content})

        response = self.llm_client.messages.create(
            model=self.model,
            max_tokens=4096,
            system=SYSTEM_PROMPT,
            messages=self.messages,
        )

        assistant_text = response.content[0].text
        self.messages.append({"role": "assistant", "content": assistant_text})

        self.total_input_tokens += response.usage.input_tokens
        self.total_output_tokens += response.usage.output_tokens

        return assistant_text

    def _call_openai(self, user_content: str, screenshot_b64: str = None) -> str:
        content = [{"type": "text", "text": user_content}]
        if screenshot_b64:
            content.append({
                "type": "image_url",
                "image_url": {"url": f"data:image/png;base64,{screenshot_b64}"}
            })

        self.messages.append({"role": "user", "content": content})

        response = self.llm_client.chat.completions.create(
            model=self.model,
            max_tokens=4096,
            messages=[{"role": "system", "content": SYSTEM_PROMPT}] + self.messages,
        )

        assistant_text = response.choices[0].message.content
        self.messages.append({"role": "assistant", "content": assistant_text})

        self.total_input_tokens += response.usage.prompt_tokens
        self.total_output_tokens += response.usage.completion_tokens

        return assistant_text

    def execute_actions(self, llm_response: str) -> str:
        """Parse and execute cu commands from LLM response."""
        results = []

        # Extract cu commands from the response (lines starting with `cu ` or in code blocks)
        lines = llm_response.split('\n')
        for line in lines:
            line = line.strip().strip('`')
            if line.startswith('cu '):
                result = self.cu(line[3:])  # Remove 'cu ' prefix
                results.append(f"$ cu {line[3:]}\n{result}")

        return '\n'.join(results) if results else "(no cu commands found in response)"

    def step(
        self,
        task_id: str,
        current_step: int,
        max_steps: int,
        env_language: str,
        task_language: str,
        task: str,
        task_step_timeout: int,
        save_dir: str,
    ) -> str:
        """Execute one step of the macOSWorld task."""

        print_message(f"Step {current_step}/{max_steps}: {task[:80]}...", title="cu_agent")

        # 1. Observe
        observation = self.observe()

        # 2. Optionally get screenshot for vision
        screenshot = None
        if current_step == 1 or "sparse" in observation.lower() or '"elements":[]' in observation:
            screenshot = self.screenshot_b64()

        # 3. Ask LLM for next action
        step_prompt = f"""Step {current_step} of {max_steps}.

{observation}

What cu commands should I run next to accomplish the task?
- Output cu commands on separate lines.
- If the task is complete, say DONE.
- If you're stuck, say FAIL."""

        llm_response = self.call_llm(task, step_prompt, screenshot)

        # 4. Check for completion
        if "DONE" in llm_response:
            print_message(f"Agent says DONE at step {current_step}", title="cu_agent")
            # Save logs
            self._save_log(save_dir, task_id, current_step, observation, llm_response, "DONE")
            return "DONE"

        if "FAIL" in llm_response:
            print_message(f"Agent says FAIL at step {current_step}", title="cu_agent")
            self._save_log(save_dir, task_id, current_step, observation, llm_response, "FAIL")
            return "FAIL"

        # 5. Execute actions
        action_results = self.execute_actions(llm_response)
        print_message(f"Actions:\n{action_results[:200]}", title="cu_agent")

        # Feed action results back as context
        self.messages.append({
            "role": "user",
            "content": f"Action results:\n{action_results}"
        })

        # 6. Save step log
        self._save_log(save_dir, task_id, current_step, observation, llm_response, action_results)

        return "unfinished"

    def _save_log(self, save_dir, task_id, step, observation, llm_response, action_results):
        """Save step log for debugging."""
        os.makedirs(save_dir, exist_ok=True)
        log_path = os.path.join(save_dir, f"step_{step:02d}.json")
        with open(log_path, 'w') as f:
            json.dump({
                "task_id": task_id,
                "step": step,
                "observation": observation[:2000],
                "llm_response": llm_response,
                "action_results": str(action_results)[:2000],
                "tokens": {
                    "input": self.total_input_tokens,
                    "output": self.total_output_tokens,
                }
            }, f, indent=2, ensure_ascii=False)
