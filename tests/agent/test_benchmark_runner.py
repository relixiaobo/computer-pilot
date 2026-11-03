"""Release task discovery must not execute benchmark recipes or partial suites."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import run as runner


class TaskDiscoveryTests(unittest.TestCase):
    def invoke(self, *args):
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch('sys.argv', ['run.py', *args]), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            try:
                runner.main()
                status = 0
            except SystemExit as exc:
                status = exc.code
        return status, stdout.getvalue(), stderr.getvalue()

    def test_default_dry_run_lists_all_three_release_tasks_without_side_effects(self):
        root = Path(__file__).resolve().parents[2]
        with contextlib.chdir(root), patch.object(runner, 'run_agent_task') as execute, patch.object(runner, 'credential_error') as credentials:
            status, output, error = self.invoke('--dry-run')
        self.assertEqual((status, error), (0, ''))
        self.assertIn('3 tasks', output)
        for task_id in ('calendar-reminder', 'finder-organize', 'mail-to-notes'):
            self.assertIn(task_id + ':', output)
        execute.assert_not_called()
        credentials.assert_not_called()

    def test_invalid_later_task_aborts_entire_suite_before_execution(self):
        valid = dict(id='test', name='Test', goal='Test', verify=[dict(command=['cu', 'apps'])])
        for invalid in (dict(goal='benchmark recipe'), [], {**valid, 'verify': []}, '{bad json'):
            with self.subTest(invalid=invalid), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / 'a.json').write_text(json.dumps(valid))
                bad = root / 'b.json'
                bad.write_text(invalid if isinstance(invalid, str) else json.dumps(invalid))
                with patch.object(runner, 'run_agent_task') as execute, patch.object(runner, 'credential_error') as credentials:
                    status, output, error = self.invoke('--tasks-dir', directory)
                self.assertEqual(status, 1)
                self.assertIn(str(bad), error)
                self.assertIn('Invalid L2 task', error)
                self.assertNotIn('Traceback', error)
                execute.assert_not_called()
                credentials.assert_not_called()

    def test_explicit_benchmark_recipe_is_rejected(self):
        recipe = Path(__file__).parent / 'tasks' / 'textedit_holdout.json'
        status, _, error = self.invoke('--task', str(recipe), '--dry-run')
        self.assertEqual(status, 1)
        self.assertIn(str(recipe), error)
        self.assertIn("'id'", error)

    def test_explicit_task_still_runs(self):
        task = Path(__file__).parent / 'tasks' / 'e2e' / 'mail_to_notes.json'
        status, output, error = self.invoke('--task', str(task), '--dry-run')
        self.assertEqual((status, error), (0, ''))
        self.assertIn('mail-to-notes:', output)
        self.assertIn('1 tasks', output)

    def test_empty_directory_is_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            status, _, error = self.invoke('--tasks-dir', directory, '--dry-run')
        self.assertEqual(status, 1)
        self.assertIn('No task files found', error)


if __name__ == '__main__':
    unittest.main()
