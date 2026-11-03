"""Offline rollout regressions for non-shell participant tool calls."""
import json
from pathlib import Path
import tempfile
import unittest

from collect_codex_benchmark import collect_turns, summarize
from benchmark_real_apps import timeline_metrics


class CollectorTests(unittest.TestCase):
    def test_image_views_count_once_and_keep_timing_without_image_payloads(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            events = [
                dict(type='turn_context', payload=dict(turn_id='turn', model='test-model', effort='high')),
                dict(type='token_usage_record', payload=dict(turn_id='turn', response_id='response',
                     usage=dict(input_tokens=100, output_tokens=10, total_tokens=110))),
                # The lower-level call is another representation of the ImageView,
                # not an additional operation to count.
                dict(type='response_item', payload=dict(type='function_call', name='exec',
                     call_id='image', arguments='await tools.view_image({path:"/tmp/capture.png"})')),
                dict(type='event_msg', payload=dict(turn_id='turn', type='item_completed',
                     started_at_ms=1000, completed_at_ms=1100, item=dict(
                         type='CommandExecution', id='shell', command='cu screenshot',
                         aggregated_output='png', status='completed', exit_code=0))),
                dict(type='event_msg', payload=dict(turn_id='turn', type='item_completed',
                     started_at_ms=1090, completed_at_ms=1110, item=dict(
                         type='ImageView', id='image', path='file:///tmp/capture.png'))),
                dict(type='event_msg', payload=dict(turn_id='turn', type='item_completed',
                     started_at_ms=1120, completed_at_ms=1130, item=dict(
                         type='McpToolCall', id='mcp', server='test', tool='observe',
                         status='failed', result=dict(isError=True, content=[dict(type='text', text='err')])))),
                dict(type='event_msg', payload=dict(turn_id='turn', type='item_completed',
                     item=dict(type='AgentMessage', id='message', text='done'))),
                dict(type='event_msg', timestamp='2026-11-03T00:00:02Z',
                     payload=dict(turn_id='turn', type='task_complete', duration_ms=1000)),
            ]
            path = root / 'rollout.jsonl'
            path.write_text('\n'.join(json.dumps(event) for event in events))
            turns = list(collect_turns(path, 'cu', 'thread'))
            self.assertEqual(len(turns), 1)
            turn = turns[0]
            summarize(turn)
            self.assertEqual(turn['tool_calls'], 3)
            self.assertEqual(turn['model_calls'], 1)
            self.assertEqual(turn['tokens']['total_tokens'], 110)
            self.assertEqual(turn['tool_errors'], 1)
            self.assertEqual(turn['tool_output_chars'], 6)
            self.assertEqual(turn['tool_interval_ms'], 120)
            self.assertEqual(turn['non_tool_interval_ms'], 880)
            image = turn['tools'][1]
            self.assertEqual(image['kind'], 'ImageView')
            self.assertEqual(image['arguments'], {'path': 'file:///tmp/capture.png'})
            self.assertEqual(image['status'], 'completed')
            self.assertEqual(image['output'], '')
            # The passive-save report must include the view without treating its
            # absent status in the original event as a tool error.
            (root / 'trace.json').write_text(json.dumps(turns))
            (root / 'saved-timeline.jsonl').write_text(json.dumps(dict(
                observed_at='1970-01-01T00:00:01.105+00:00', all_exact=True)))
            metrics = timeline_metrics(root)
            self.assertEqual(metrics['post_save_tool_calls'], 2)
            self.assertEqual(metrics['post_save_tool_errors'], 1)

    def test_completed_image_view_is_retained_in_an_interrupted_turn(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'rollout.jsonl'
            events = [
                dict(type='event_msg', payload=dict(turn_id='turn', type='item_completed',
                     started_at_ms=1000, completed_at_ms=1000, item=dict(
                         type='ImageView', id='image', path='file:///tmp/capture.png'))),
                dict(type='event_msg', timestamp='2026-11-03T00:00:02Z',
                     payload=dict(turn_id='turn', type='turn_aborted', duration_ms=1000)),
            ]
            path.write_text('\n'.join(json.dumps(event) for event in events))
            turn = next(iter(collect_turns(path, 'cu', 'thread')))
            summarize(turn)
            self.assertEqual(turn['tool_calls'], 1)
            self.assertEqual(turn['tool_errors'], 0)
            self.assertEqual(turn['tool_interval_ms'], 0)
            self.assertEqual(turn['non_tool_interval_ms'], 1000)
            self.assertNotIn('elapsed_ms', turn)


if __name__ == '__main__':
    unittest.main()
