"""Offline protocol regressions; no GUI, model calls or credentials."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import benchmark_varied_workflows as bench


class ProtocolTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.settings = dict(model='test-model', reasoning_effort='high', service_tier=None)
        self.sequence = [dict(label=f'triage-{i}-{arm}', mode='triage', repeat=i, arm=arm)
                         for i in (1, 2) for arm in ('cu', 'native')]
        self.write(self.root / 'manifest.json', dict(sequence=self.sequence, settings=self.settings))

    def write(self, path, value):
        path.write_text(json.dumps(value))

    def sample(self, label, *, passed=True, audited=True, elapsed=1000, **changes):
        entry = next(e for e in self.sequence if e['label'] == label)
        trial = self.root / label
        trial.mkdir()
        result = dict(entry, thread_id=label, source_sha256='paired-source',
                      evidence_sha256={'source.json': 'paired-source'},
                      setup=[dict(self.settings, elapsed_ms=100)],
                      runs=[dict(self.settings, elapsed_ms=elapsed)],
                      setup_ms=100, execution_ms=elapsed, combined_turn_ms=100 + elapsed,
                      verification=dict(passed=passed))
        result.update(changes)
        self.write(trial / 'result.json', result)
        if audited:
            self.write(trial / 'audit.json', dict(compliant=True, note='Reviewed setup and GO tools'))
        return trial, result

    def report(self):
        with contextlib.redirect_stdout(io.StringIO()):
            bench.report(self.root)
        return json.loads((self.root / 'report.json').read_text())

    def test_failures_remain_visible_but_do_not_become_fast_successes(self):
        self.sample('triage-1-cu', elapsed=2000)
        self.sample('triage-1-native', elapsed=4000)
        self.sample('triage-2-cu', passed=False, elapsed=10)
        self.sample('triage-2-native', elapsed=1000)
        report = self.report()
        group = report['groups'][0]
        self.assertEqual(group['arms']['cu']['counts']['failed'], 1)
        self.assertEqual(group['arms']['cu']['success_medians_ms']['execution_ms'], 2000)
        self.assertEqual(group['median_paired_go_ratio'], 0.5)
        self.assertEqual(len(report['attempts']), 4)
        self.assertEqual(report['attempts'][2]['execution_ms'], 10)

    def test_missing_audit_and_unrun_trials_cannot_enter_comparison(self):
        self.sample('triage-1-cu', audited=False)
        self.sample('triage-1-native')
        result = self.report()
        self.assertEqual([r['status'] for r in result['attempts']],
                         ['unreviewed', 'passed', 'pending', 'pending'])
        self.assertIsNone(result['groups'][0]['median_paired_go_ratio'])

    def test_settings_and_isolation_deviations_are_not_task_failures(self):
        trial, result = self.sample('triage-1-cu')
        result['setup'][0]['model'] = 'different-model'
        self.write(trial / 'result.json', result)
        trial, _ = self.sample('triage-1-native')
        self.write(trial / 'audit.json', dict(compliant=False, note='Used the wrong backend'))
        result = self.report()
        self.assertEqual([r['status'] for r in result['attempts'][:2]], ['excluded', 'excluded'])
        self.assertIn('wrong backend', result['attempts'][1]['reasons'][0])

    def test_reused_task_and_changed_input_are_excluded(self):
        self.sample('triage-1-cu', thread_id='same-task')
        self.sample('triage-1-native', thread_id='same-task')
        self.sample('triage-2-cu', source_sha256='different-source')
        self.sample('triage-2-native')
        result = self.report()
        self.assertTrue(all(r['status'] == 'excluded' for r in result['attempts']))
        self.assertEqual(result['groups'][0]['successful_pairs'], [])

    def test_prepare_counterbalances_each_dataset_without_gui(self):
        target = self.root / 'new'
        with patch.object(bench.subprocess, 'run'), \
                patch.object(bench.subprocess, 'check_output', return_value='commit\n'), \
                patch.object(bench, 'fingerprints', return_value={'inputs': 'sha'}), \
                contextlib.redirect_stdout(io.StringIO()):
            bench.prepare(target, ['triage'], 4, 'test-model', 'high', None)
        manifest = json.loads((target / 'manifest.json').read_text())
        self.assertEqual([r['arm'] for r in manifest['sequence']],
                         ['native', 'cu', 'cu', 'native', 'native', 'cu', 'cu', 'native'])
        self.assertEqual(manifest['settings'], self.settings)

    def test_single_turn_report_counts_initialization_in_full_task_time(self):
        self.write(self.root / 'manifest.json', dict(sequence=self.sequence, settings=self.settings, single_turn=True))
        self.sample('triage-1-cu', setup=[], setup_ms=0, elapsed=3000, combined_turn_ms=3000)
        self.sample('triage-1-native', setup=[], setup_ms=0, elapsed=2000, combined_turn_ms=2000)
        result = self.report()
        self.assertEqual(result['groups'][0]['median_paired_combined_ratio'], 1.5)
        self.assertIsNone(result['groups'][0]['median_paired_go_ratio'])
        markdown = (self.root / 'report.md').read_text()
        self.assertIn('Full task s', markdown)
        self.assertNotIn('GO s', markdown)

    def test_single_turn_collection_needs_no_go_marker(self):
        self.write(self.root / 'manifest.json', dict(sequence=self.sequence, settings=self.settings, single_turn=True))
        trial = self.root / 'triage-1-cu'
        trial.mkdir()
        self.write(trial / 'meta.json', dict(self.sequence[0], source_sha256='source'))
        (trial / 'thread.txt').write_text('continuous')
        (self.root / 'rollout-continuous.jsonl').write_text('')
        self.write(trial / 'source.json', bench.source('triage', 1))
        turn = dict(self.settings, elapsed_ms=3000, tools=[], usage_records={}, turn_id='one')
        with patch.object(bench, 'collect_turns', return_value=[turn]), contextlib.redirect_stdout(io.StringIO()):
            bench.collect(self.root, 'triage-1-cu', self.root)
        result = json.loads((trial / 'result.json').read_text())
        self.assertEqual(result['setup'], [])
        self.assertEqual(len(result['runs']), 1)
        self.assertEqual(result['combined_turn_ms'], 3000)
        self.assertEqual(result['runs'][0]['label'], 'triage-1-cu')

    def test_collect_preserves_setup_and_all_go_tools(self):
        trial = self.root / 'triage-1-cu'
        trial.mkdir()
        self.write(trial / 'meta.json', dict(self.sequence[0], source_sha256='source'))
        (trial / 'thread.txt').write_text('thread')
        (self.root / 'rollout-thread.jsonl').write_text('')
        self.write(trial / 'source.json', bench.source('triage', 1))
        turns = [dict(self.settings, elapsed_ms=100, tools=[], usage_records={}, turn_id=str(i))
                 for i in range(3)]
        for turn in turns[1:]:
            turn['label'] = 'cu-1'
        turns.append(dict(turn_id='interrupted', started_at='2026-10-31T00:00:00Z',
                          tools=[dict(command='unfinished operation')], usage_records={}))
        with patch.object(bench, 'collect_turns', return_value=turns), \
                contextlib.redirect_stdout(io.StringIO()):
            bench.collect(self.root, 'triage-1-cu', self.root)
        trace = json.loads((trial / 'trace.json').read_text())
        result = json.loads((trial / 'result.json').read_text())
        self.assertEqual(len(trace), 4)
        self.assertEqual(trace[-1]['tools'][0]['command'], 'unfinished operation')
        self.assertEqual(result['incomplete_turns'], ['interrupted'])
        self.assertEqual(len(result['setup']), 1)
        self.assertEqual(len(result['runs']), 2)
        self.assertEqual(result['combined_turn_ms'], 300)
        self.assertFalse(result['verification']['passed'])

    def test_interrupted_task_keeps_duration_and_order_without_zero_success_time(self):
        self.write(self.root / 'manifest.json', dict(sequence=self.sequence, settings=self.settings, single_turn=True))
        trial = self.root / 'triage-1-cu'
        trial.mkdir()
        self.write(trial / 'meta.json', dict(self.sequence[0], source_sha256='source'))
        (trial / 'thread.txt').write_text('aborted')
        self.write(trial / 'source.json', bench.source('triage', 1))
        events = [
            dict(type='event_msg', timestamp='2026-10-31T00:00:00Z',
                 payload=dict(turn_id='one', type='task_started')),
            dict(type='turn_context', payload=dict(turn_id='one', model='test-model', effort='high')),
            dict(type='event_msg', timestamp='2026-10-31T00:00:03Z',
                 payload=dict(turn_id='one', type='turn_aborted', duration_ms=3000)),
        ]
        (self.root / 'rollout-aborted.jsonl').write_text('\n'.join(json.dumps(e) for e in events))
        with contextlib.redirect_stdout(io.StringIO()):
            bench.collect(self.root, 'triage-1-cu', self.root)
        self.sample('triage-1-native', setup=[], setup_ms=0,
                    runs=[dict(self.settings, elapsed_ms=2000, started_at='2026-10-31T00:00:04Z')],
                    execution_ms=2000, combined_turn_ms=2000)
        report = self.report()
        interrupted = report['attempts'][0]
        self.assertEqual(interrupted['status'], 'excluded')
        self.assertEqual(interrupted['interruptions'][0]['interrupted_elapsed_ms'], 3000)
        self.assertEqual(report['actual_order'], ['triage-1-cu', 'triage-1-native'])
        self.assertEqual(report['groups'][0]['successful_pairs'], [])
        markdown = (self.root / 'report.md').read_text()
        self.assertIn('| triage-1-cu | excluded | — |', markdown)
        self.assertIn('Interrupted after 3.000s', markdown)


if __name__ == '__main__':
    unittest.main()
