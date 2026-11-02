"""Offline saved-result checks for the real application workflows."""
import json
import io
from contextlib import redirect_stdout
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import benchmark_real_apps as bench


class RealAppResults(unittest.TestCase):
    def test_prepared_repetitions_keep_complete_pairs_and_reproducible_order(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            app = root / 'App'
            (app / 'Contents').mkdir(parents=True)
            (app / 'Contents/Info.plist').write_bytes(bench.plistlib.dumps({'CFBundleShortVersionString': 'test'}))
            with patch.dict(bench.APPS, {'textedit': (str(app), 'TextEdit')}), patch.object(bench, 'fingerprints', return_value={}), patch.object(bench.subprocess, 'check_output', return_value='test-sha'), redirect_stdout(io.StringIO()):
                bench.prepare(root / 'a', 'holdout', repeats=3, seed=42)
                bench.prepare(root / 'b', 'holdout', repeats=3, seed=42)
            a = json.loads((root / 'a/manifest.json').read_text())
            b = json.loads((root / 'b/manifest.json').read_text())
            self.assertEqual(a['sequence'], b['sequence'])
            self.assertEqual(len({row['label'] for row in a['sequence']}), 6)
            for repeat in (1, 2, 3):
                self.assertEqual({row['arm'] for row in a['sequence'] if row['repeat'] == repeat}, {'cu', 'native'})

    def test_finder_tasks_verify_exact_paths_and_bytes(self):
        for scenario in ('organize', 'rename'):
            spec = bench.source('finder', 'finder', scenario)
            self.assertEqual(spec['mode'], 'finder')
            with tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / 'saved.json'
                path.write_text(json.dumps({'files': spec['expected_files']}))
                self.assertTrue(bench.verify(spec, path)['passed'])
                changed = dict(spec['expected_files'])
                name = next(iter(changed))
                changed['wrong/' + name] = changed.pop(name)
                path.write_text(json.dumps({'files': changed}))
                self.assertFalse(bench.verify(spec, path)['passed'])

    def test_finder_copy_instead_of_move_fails_final_and_timeline_checks(self):
        spec = bench.source('finder', 'finder', 'organize')
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            trial = root / 'trial'
            trial.mkdir()
            work = trial / 'workspace'
            work.mkdir()
            meta = dict(mode='finder', workspace=str(work))
            (trial / 'meta.json').write_text(json.dumps(meta))
            (trial / 'source.json').write_text(json.dumps(spec))
            for name, text in {**spec['files'], **spec['expected_files']}.items():
                path = work / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text)
            bench.capture_saved(root, 'trial')
            self.assertFalse(bench.verify(spec, trial / 'saved.json')['passed'])
            self.assertFalse(bench.saved_file_sample(meta, spec)['all_exact'])
            for name in set(spec['files']) - set(spec['expected_files']):
                (work / name).unlink()
            bench.capture_saved(root, 'trial')
            self.assertTrue(bench.verify(spec, trial / 'saved.json')['passed'])
            self.assertTrue(bench.saved_file_sample(meta, spec)['all_exact'])
            (work / 'extra.bin').write_bytes(b'\xff')
            (work / 'unrelated-link').symlink_to(root / 'absent')
            bench.capture_saved(root, 'trial')
            self.assertFalse(bench.verify(spec, trial / 'saved.json')['passed'])

    def test_finder_cleanup_uses_owned_target_url_after_navigation(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            trial = root / 'trial'
            trial.mkdir()
            meta = dict(pid=123, mode='finder', shared_app=True,
                        workspace=str(root / 'owned space'), runtime_home=folder, client_key='test')
            path = trial / 'meta.json'
            path.write_text(json.dumps(meta))
            broker = SimpleNamespace(returncode=0, stdout='{"running":false}')
            with patch.object(bench, 'jxa') as jxa, patch.object(bench.subprocess, 'run', return_value=broker):
                bench.stop(root, 'trial')
                script = jxa.call_args.args[0]
                self.assertIn((root / 'owned space').resolve().as_uri() + '/', script)
                self.assertIn('w.target.url()', script)
                self.assertIsNone(json.loads(path.read_text())['pid'])
            path.write_text(json.dumps(meta))
            with patch.object(bench, 'jxa', side_effect=RuntimeError('Test Finder window remains open')), patch.object(bench.subprocess, 'run', return_value=broker):
                with self.assertRaisesRegex(RuntimeError, 'remains open'):
                    bench.stop(root, 'trial')
                self.assertEqual(json.loads(path.read_text())['pid'], 123)

    def test_timeline_metrics_count_calls_after_first_exact_sample(self):
        with tempfile.TemporaryDirectory() as folder:
            trial = Path(folder)
            (trial / 'saved-timeline.jsonl').write_text(
                '\n'.join(json.dumps(row) for row in (
                    {'observed_at': '2026-11-01T00:00:00+00:00', 'all_exact': False},
                    {'observed_at': '2026-11-01T00:00:02+00:00', 'all_exact': True},
                    {'observed_at': '2026-11-01T00:00:03+00:00', 'all_exact': True},
                )) + '\n')
            (trial / 'trace.json').write_text(json.dumps([{'tools': [
                {'end_ms': 1793491201000, 'status': 'completed', 'exit_code': 0},
                {'end_ms': 1793491202500, 'status': 'completed', 'exit_code': 0},
                {'end_ms': 1793491203000, 'status': 'failed', 'exit_code': 1},
            ]}]))
            self.assertEqual(bench.timeline_metrics(trial), {
                'save_observation_count': 3, 'first_exact_index': 1,
                'post_save_tool_calls': 2, 'post_save_tool_errors': 1,
            })

    def test_backend_call_estimate_excludes_launcher_setup_and_counts_batches(self):
        traces = [{"tools": [
            {"command": ["/bin/zsh", "-lc", "cat > ./pilot <<'EOF'\\nEOF\\nchmod 700 ./pilot\\n./pilot snapshot"]},
            {"command": ["/bin/zsh", "-lc", "set -e\\n./pilot key down\\n./pilot key enter"]},
        ]}]
        self.assertEqual(bench.shared.backend_calls_estimate(traces, 'cu'), 3)
        native = [{"tools": [
            {"server": "cua_repl"}, {"server": "cua_repl"},
            {"server": "other"},
        ]}]
        self.assertEqual(bench.shared.backend_calls_estimate(native, 'native'), 2)

    def test_varied_editor_sources_keep_repeated_sections_scoped(self):
        for scenario in ('json', 'markdown', 'multi'):
            spec = bench.source('vscode', 'varied', scenario)
            self.assertEqual(set(spec['files']), set(spec['expected_files']))
            with tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / 'saved.json'
                path.write_text(json.dumps(dict(files=spec['expected_files'])))
                self.assertTrue(bench.verify(spec, path)['passed'])
                name = next(iter(spec['files']))
                changed = dict(spec['expected_files'])
                changed[name] = changed[name].replace('release.', 'wrong.', 1)
                if changed[name] == spec['expected_files'][name]:
                    changed[name] = changed[name].replace('production', 'wrong-environment', 1)
                if changed[name] == spec['expected_files'][name]:
                    changed[name] = changed[name].replace('us-west', 'wrong-region', 1)
                if changed[name] == spec['expected_files'][name]:
                    changed[name] = changed[name].replace('ready', 'wrong-status', 1)
                self.assertNotEqual(changed[name], spec['expected_files'][name])
                path.write_text(json.dumps(dict(files=changed)))
                self.assertFalse(bench.verify(spec, path)['passed'])

    def test_editor_cases_track_correct_saved_bytes_and_later_regressions(self):
        for case in ('local', 'scoped', 'multi'):
            spec = bench.source('vscode', 'editor', case)
            with tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                meta = dict(workspace=folder, files={name: name for name in spec['files']})
                self.assertFalse(bench.saved_file_sample(meta, spec)['all_exact'])
                for name, content in spec['files'].items():
                    (root / name).write_text(content)
                self.assertFalse(bench.saved_file_sample(meta, spec)['all_exact'])
                for name, content in spec['expected_files'].items():
                    (root / name).write_text(content)
                self.assertTrue(bench.saved_file_sample(meta, spec)['all_exact'])
                saved = root / 'saved.json'
                saved.write_text(json.dumps(dict(files=spec['expected_files'])))
                self.assertTrue(bench.verify(spec, saved)['passed'])
                name = next(iter(spec['files']))
                (root / name).write_text(spec['expected_files'][name].rstrip('\n'))
                self.assertFalse(bench.saved_file_sample(meta, spec)['files'][name]['exact'])
                # A visually similar CRLF rewrite is not byte-preserving.
                (root / name).write_bytes(spec['expected_files'][name].replace('\n', '\r\n').encode())
                self.assertFalse(bench.saved_file_sample(meta, spec)['all_exact'])
                trial = root / 'trial'
                trial.mkdir()
                (trial / 'meta.json').write_text(json.dumps(dict(mode='vscode', **meta)))
                (trial / 'source.json').write_text(json.dumps(spec))
                bench.capture_saved(root, 'trial')
                self.assertFalse(bench.verify(spec, trial / 'saved.json')['passed'])

    def test_scoped_file_tasks_reject_out_of_scope_edits_and_lost_newlines(self):
        for mode in ('textedit', 'vscode'):
            spec = bench.source(mode, 'scoped')
            with tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / 'saved.json'
                files = dict(spec['expected_files'])
                path.write_text(json.dumps(dict(files=files)))
                self.assertTrue(bench.verify(spec, path)['passed'])
                name = 'brief.txt' if mode == 'textedit' else 'release.md'
                # Editing a repeated historical value violates the scoped task.
                files[name] = files[name].replace('草案', '待验收').replace('Channel: preview', 'Channel: live')
                path.write_text(json.dumps(dict(files=files)))
                self.assertFalse(bench.verify(spec, path)['passed'])
                files[name] = spec['expected_files'][name].rstrip('\n')
                path.write_text(json.dumps(dict(files=files)))
                self.assertFalse(bench.verify(spec, path)['passed'])

    def test_scoped_numbers_requires_discount_order_and_live_formulas(self):
        spec = bench.source('numbers', 'scoped')
        recalculated = [round((row[2]+1)*(row[3]+2)*.9, 2) for row in spec['expected_rows'][1:7]]
        result = dict(saved_file_exists=True, values=[list(row) for row in spec['expected_rows']],
                      formulas=['=ROUND(C2*D2*0.9,2)']*6+['=SUM(E2:E7)'],
                      recalculated_totals=recalculated+[round(sum(recalculated),2)],
                      sheet='Invoice', row_count=8, column_count=5)
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'saved.json'
            path.write_text(json.dumps(result))
            self.assertTrue(bench.verify(spec,path)['passed'])
            result['recalculated_totals']=[row[4] for row in result['values'][1:]]
            path.write_text(json.dumps(result))
            self.assertFalse(bench.verify(spec,path)['checks']['formulas_recalculate'])
            result['values'][1],result['values'][2]=result['values'][2],result['values'][1]
            path.write_text(json.dumps(result))
            self.assertFalse(bench.verify(spec,path)['checks']['cells_exact'])

    def test_numbers_setup_refuses_an_existing_process_before_launch(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder)
            (root/'manifest.json').write_text(json.dumps(dict(fingerprints={},sequence=[dict(mode='numbers',label='numbers-1-cu')])))
            with patch.object(bench,'fingerprints',return_value={}), patch.object(bench,'process_ids',return_value=[123]), patch.object(bench.subprocess,'run') as launch:
                with self.assertRaisesRegex(RuntimeError,'already running'):
                    bench.start(root,'numbers-1-cu')
                launch.assert_not_called()
                self.assertFalse((root/'numbers-1-cu').exists())

    def test_completion_claims_do_not_override_artifacts_or_exclusions(self):
        rows = [dict(status='passed', audit=dict(claimed_complete=True)),
                dict(status='failed', audit=dict(claimed_complete=True)),
                dict(status='failed', audit=dict(claimed_complete=False)),
                dict(status='failed', audit={}),
                dict(status='excluded', audit=dict(claimed_complete=True))]
        summary = bench.accuracy_summary(rows)
        self.assertEqual(summary['success_rate'], 1/5)
        self.assertEqual(summary['accepted_completed'], 4)
        self.assertEqual(summary['completion_claims_reviewed'], 3)
        self.assertEqual(summary['false_completion_claims'], 1)
        self.assertEqual(summary['false_completion_rate'], 1/2)
        self.assertIsNone(bench.accuracy_summary([])['false_completion_rate'])

    def test_alternate_dataset_cannot_pass_with_baseline_or_familiar_url(self):
        spec = bench.source('vscode', 'alternate')
        config = json.loads(spec['files']['service.json'])
        config.update(environment='production', timeoutSeconds=45, debug=False)
        config['endpoint'] = config['endpoint'].replace('sandbox', 'production')
        for worker in config['workers']: worker['retries'] = 4
        files = {name: text.replace('sandbox', 'production').replace('30 seconds', '45 seconds')
                 .replace('Status: draft', 'Status: ready').replace('[ ]', '[x]')
                 for name, text in spec['files'].items()}
        files['service.json'] = json.dumps(config)
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'saved.json'
            path.write_text(json.dumps(dict(files=files)))
            self.assertTrue(bench.verify(spec, path)['passed'])
            self.assertFalse(bench.verify(bench.source('vscode'), path)['passed'])
            config['endpoint'] = 'https://production.example.com/api'
            files['service.json'] = json.dumps(config)
            path.write_text(json.dumps(dict(files=files)))
            self.assertFalse(bench.verify(spec, path)['checks']['config_exact'])
        for mode in bench.APPS:
            self.assertNotEqual(bench.source(mode), bench.source(mode, 'alternate'))

    def test_save_dialog_keeps_shared_window_ownership_for_cleanup(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);trial=root/'vscode-1-cu';trial.mkdir()
            meta=dict(pid=123,shared_app=True,mode='vscode',workspace=str(root/'CP-task'),
                      runtime_home=str(root/'runtime'),client_key='test',files={})
            path=trial/'meta.json';path.write_text(json.dumps(meta))
            remaining=SimpleNamespace(returncode=0,stdout=json.dumps(dict(windows=[dict(index=2,title='file — CP-task')])))
            closed=SimpleNamespace(returncode=0,stdout=json.dumps(dict(windows=[])))
            dispatched=SimpleNamespace(returncode=0,stdout='{"ok":true}')
            broker=SimpleNamespace(returncode=0,stdout='{"running":true,"pid":456}')
            with patch.object(bench.subprocess,'run',side_effect=[remaining,dispatched,remaining,broker]), patch.object(bench.os,'kill') as kill, patch.object(bench.time,'monotonic',side_effect=[0,3]):
                with self.assertRaisesRegex(RuntimeError,'Test window remains open'):
                    bench.stop(root,trial.name)
                self.assertEqual(json.loads(path.read_text())['pid'],123)
                kill.assert_called_once_with(456,bench.signal.SIGTERM)
            # A coordinator handles the task-only dialog, then retries cleanup.
            with patch.object(bench.subprocess,'run',side_effect=[closed,broker]), patch.object(bench.os,'kill') as kill:
                bench.stop(root,trial.name)
                self.assertIsNone(json.loads(path.read_text())['pid'])
                kill.assert_called_once_with(456,bench.signal.SIGTERM)
            # A normal asynchronous close gets time to finish without another click.
            path.write_text(json.dumps(meta))
            with patch.object(bench.subprocess,'run',side_effect=[remaining,dispatched,remaining,closed,broker]), patch.object(bench.os,'kill'), patch.object(bench.time,'sleep'):
                bench.stop(root,trial.name)
                self.assertIsNone(json.loads(path.read_text())['pid'])

    def check(self, mode, result):
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'saved.json'
            path.write_text(json.dumps(result))
            return bench.verify(bench.source(mode), path)

    def test_missing_saved_outputs_cannot_pass(self):
        for mode in bench.APPS:
            self.assertFalse(self.check(mode, {})['passed'])

    def test_numbers_checks_saved_values_formulas_shape_and_order(self):
        result=dict(saved_file_exists=True,sheet='Budget',row_count=8,column_count=5,
            values=[['Item','Team','Quantity','Unit price','Total'],
                    ['Adapter','Office',5,15,75],['Hosting','Platform',10,25,250],
                    ['Keyboard','Office',4,45,180],['Monitor','Office',2,195,390],
                    ['Storage','Platform',6,30,180],['Training','People',3,120,360],
                    ['Grand total',None,None,None,1435]],
            formulas=[f'=C{i}*D{i}' for i in range(2,8)]+['=SUM(E2:E7)'],
            recalculated_totals=[102,297,235,591,224,488,1937])
        self.assertTrue(self.check('numbers',result)['passed'])
        result['recalculated_totals'][0]=75
        self.assertFalse(self.check('numbers',result)['checks']['formulas_recalculate'])
        result['recalculated_totals'][0]=102
        result['formulas'][0]=None
        self.assertFalse(self.check('numbers',result)['passed'])
        result['formulas'][0]='=C2*D2'
        result['values'][1],result['values'][2]=result['values'][2],result['values'][1]
        self.assertFalse(self.check('numbers',result)['passed'])

    def test_vscode_requires_all_files_and_preserves_other_values(self):
        spec=bench.source('vscode');config=json.loads(spec['files']['service.json'])
        config.update(environment='production',endpoint='https://production.example.test/api',timeoutSeconds=45,debug=False)
        for worker in config['workers']:worker['retries']=4
        files={'service.json':json.dumps(config),'release.md':'# production release\n\nDeploy to https://production.example.test/api.\nTimeout: 45 seconds.\nStatus: ready\n\nKeep this note: “中文验收” — build 17.\n',
               'checklist.txt':'production deployment\n[x] verify endpoint\n[x] verify timeout\n[x] disable debug\n[x] confirm workers\n'}
        self.assertTrue(self.check('vscode',dict(files=files))['passed'])
        config['workers'][2]['retries']=2;files['service.json']=json.dumps(config)
        self.assertFalse(self.check('vscode',dict(files=files))['passed'])


if __name__=='__main__':unittest.main()
