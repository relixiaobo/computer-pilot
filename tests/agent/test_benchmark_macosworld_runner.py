"""macOSWorld provider calls must fail within a bounded relay timeout."""

import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/macosworld"))
import run_benchmark  # noqa: E402


class MacOSWorldRunnerTimeoutTests(unittest.TestCase):
    def test_default_timeout_is_bounded(self):
        with patch.dict(os.environ, {}, clear=True):
            self.assertEqual(run_benchmark.llm_timeout_seconds(), 120.0)

    def test_invalid_or_nonpositive_timeout_uses_default(self):
        for value in ("bad", "0", "-5"):
            with self.subTest(value=value), patch.dict(os.environ, {"MACOSWORLD_LLM_TIMEOUT": value}):
                self.assertEqual(run_benchmark.llm_timeout_seconds(), 120.0)

    def test_provider_clients_receive_configured_timeout(self):
        created = []

        class FakeOpenAIResponse:
            choices = [type("Choice", (), {"message": type("Message", (), {"content": "DONE"})()})()]
            usage = type("Usage", (), {"prompt_tokens": 2, "completion_tokens": 1})()

        class FakeOpenAI:
            def __init__(self, **kwargs):
                self.kwargs = kwargs
                created.append(("openai", kwargs))
                self.chat = type("Chat", (), {"completions": type("Completions", (), {
                    "create": lambda _self, **_kwargs: FakeOpenAIResponse()
                })()})()

        class FakeAnthropicResponse:
            content = [type("Content", (), {"text": "DONE"})()]
            usage = type("Usage", (), {"input_tokens": 2, "output_tokens": 1})()

        class FakeAnthropic:
            def __init__(self, **kwargs):
                self.kwargs = kwargs
                created.append(("anthropic", kwargs))
                self.messages = type("Messages", (), {
                    "create": lambda _self, **_kwargs: FakeAnthropicResponse()
                })()

        with patch.dict(os.environ, {"MACOSWORLD_LLM_TIMEOUT": "17.5"}), \
                patch.dict(sys.modules, {
                    "openai": type("OpenAIModule", (), {"OpenAI": FakeOpenAI}),
                    "anthropic": type("AnthropicModule", (), {"Anthropic": FakeAnthropic}),
                }):
            self.assertEqual(run_benchmark._call_openai("model", [])[0], "DONE")
            self.assertEqual(run_benchmark._call_anthropic("model", [])[0], "DONE")
        self.assertEqual(created, [("openai", {"timeout": 17.5}), ("anthropic", {"timeout": 17.5})])

    def test_llm_error_skips_grading_and_cannot_become_a_pass(self):
        task = {
            "id": "timeout-task",
            "task": {"en": "Timed out task"},
            "grading_command": [["true"]],
        }
        with patch.object(run_benchmark, "cu_raw", return_value="{}"), \
                patch.object(run_benchmark, "call_llm", side_effect=TimeoutError("relay timeout")), \
                patch.object(run_benchmark.time, "sleep"), \
                patch.object(run_benchmark.subprocess, "run") as grader:
            result = run_benchmark.run_task(task, "gpt-6.1-sol", max_steps=1, verbose=False)
        self.assertEqual(result["status"], "error")
        self.assertFalse(result["grade"])
        grader.assert_not_called()


if __name__ == "__main__":
    unittest.main()
