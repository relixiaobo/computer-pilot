"""Resource-budget regressions; no model, application or user clipboard access."""
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

from collect_codex_benchmark import UsageReader, summarize
from run_codex_comparison import run_participant


class ResourceBudgetTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root/'sessions').mkdir()
        self.rollout = self.root/'sessions/rollout.jsonl'
        self.rollout.write_bytes(b'')
        (self.root/'events.jsonl').write_text('')
        self.manifest = dict(budget=dict(model_calls=2, total_tokens=100), inactivity_timeout_seconds=1200)

    def record(self, response, **usage):
        return json.dumps(dict(type='token_usage_record', payload=dict(response_id=response, usage=usage)))+'\n'

    def process(self, waits):
        return Mock(pid=123, stdin=io.StringIO(), wait=Mock(side_effect=waits))

    def test_usage_deduplicates_responses_and_waits_for_complete_utf8_lines(self):
        first = self.record('one', input_tokens=60, cached_input_tokens=50,
                            output_tokens=10, reasoning_output_tokens=7, total_tokens=70)
        next_line = self.record('two', input_tokens=20, cached_input_tokens=10,
                                output_tokens=5, reasoning_output_tokens=3, total_tokens=25)
        self.rollout.write_text(first+first+next_line[:-1])
        reader = UsageReader(self.rollout)
        self.assertEqual(reader.read()['model_calls'], 1)
        with self.rollout.open('ab') as f:
            f.write(b'\n'+b'{"type":"response_item","payload":{"text":"\xe4')
        summary = reader.read()
        self.assertEqual(summary['model_calls'], 2)
        self.assertEqual(summary['tokens']['total_tokens'], 95)
        self.assertEqual(summary['tokens']['reasoning_output_tokens'], 10)
        self.assertEqual(summary['tokens']['output_tokens'], 15)
        self.assertEqual(reader.read(), summary)

    def test_each_budget_can_stop_the_runner_without_counting_tools_as_rounds(self):
        for budget, usage, exceeded in (
            (dict(model_calls=1,total_tokens=100), dict(total_tokens=10), 'model_calls'),
            (dict(model_calls=10,total_tokens=100), dict(total_tokens=100), 'total_tokens'),
        ):
            with self.subTest(exceeded=exceeded):
                self.rollout.write_text(self.record('one', **usage))
                process = self.process([subprocess.TimeoutExpired('test', 1), 0])
                with patch('run_codex_comparison.os.killpg') as stop:
                    run_participant(process, 'task', self.root, self.root, dict(self.manifest, budget=budget))
                result = json.loads((self.root/'stop.json').read_text())
                self.assertEqual(result['kind'], 'budget_exhausted')
                self.assertEqual(result['exceeded'], [exceeded])
                self.assertEqual(result['observed']['model_calls'], 1)
                stop.assert_called_once()

    def test_slow_response_does_not_become_a_600_second_task_failure(self):
        self.rollout.write_text(self.record('one', total_tokens=10))
        process = self.process([subprocess.TimeoutExpired('test', 1), 0])
        with patch('run_codex_comparison.time.monotonic', side_effect=[0, 700]), patch('run_codex_comparison.os.killpg') as stop:
            run_participant(process, 'task', self.root, self.root, self.manifest)
        stop.assert_not_called()
        self.assertFalse((self.root/'stop.json').exists())

    def test_final_response_at_budget_can_exit_normally(self):
        self.rollout.write_text(self.record('one', total_tokens=100)+
            json.dumps(dict(type='event_msg', payload=dict(type='task_complete')))+'\n')
        process = self.process([subprocess.TimeoutExpired('test', 1), 0])
        with patch('run_codex_comparison.os.killpg') as stop:
            run_participant(process, 'task', self.root, self.root, self.manifest)
        stop.assert_not_called()

    def test_inactivity_is_infrastructure_failure_and_retains_usage(self):
        self.rollout.write_text(self.record('one', total_tokens=10))
        process = self.process([subprocess.TimeoutExpired('test', 1), subprocess.TimeoutExpired('test', 1), 0])
        with patch('run_codex_comparison.time.monotonic', side_effect=[0, 1, 1202]), patch('run_codex_comparison.os.killpg'):
            run_participant(process, 'task', self.root, self.root, self.manifest)
        result = json.loads((self.root/'stop.json').read_text())
        self.assertEqual(result['kind'], 'infrastructure_stall')
        self.assertEqual(result['observed']['tokens']['total_tokens'], 10)

    def test_interrupted_summary_keeps_tokens_without_inventing_complete_time(self):
        turn = dict(usage_records={'response':dict(input_tokens=60, cached_input_tokens=50,
                    output_tokens=10, reasoning_output_tokens=7, total_tokens=70)},
                    tools=[], interrupted_elapsed_ms=9000)
        summarize(turn)
        self.assertEqual(turn['model_calls'], 1)
        self.assertEqual(turn['tokens']['total_tokens'], 70)
        self.assertEqual(turn['non_tool_interval_ms'], 9000)
        self.assertNotIn('elapsed_ms', turn)


if __name__ == '__main__':
    unittest.main()
