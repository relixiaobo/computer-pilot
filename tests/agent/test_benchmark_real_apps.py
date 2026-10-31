"""Offline saved-result checks for the real application workflows."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import benchmark_real_apps as bench


class RealAppResults(unittest.TestCase):
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
            formulas=[f'=C{i}*D{i}' for i in range(2,8)]+['=SUM(E2:E7)'])
        self.assertTrue(self.check('numbers',result)['passed'])
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
