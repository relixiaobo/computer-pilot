"""Reviewed L2 evidence cannot hide failed tasks or outlive tested sources."""
import importlib.util
import json
from pathlib import Path
import hashlib
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('evidence', ROOT / 'scripts/check-agent-evidence.py')
evidence = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evidence)


class EvidenceFixture(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        files = {
            'Cargo.toml': '[package]\nname = "cu"\nversion = "0.9.3"\n',
            'Cargo.lock': '# test lock\n',
            'src/main.rs': '// test runtime\n',
            'tests/agent/run.py': 'raise RuntimeError("API runner unexpectedly invoked")\n',
            'plugin/skills/computer-pilot/SKILL.md': 'test skill\n',
            'plugin/skills/computer-pilot/references/scripting.md': 'test reference\n',
            'plugin/skills/computer-pilot/compatibility.json': '{"version":"0.9.3"}',
        }
        results = []
        for name in ('mail_to_notes', 'calendar_reminder', 'finder_organize'):
            task = dict(id=name, name=name, goal='Test', verify=[dict(description='Expected output')])
            files['tests/agent/tasks/e2e/' + name + '.json'] = json.dumps(task)
            results.append(dict(task_id=name, model='test-model', agent_status='done', verified=True,
                                cleanup_errors=[], checks=[dict(check='Expected output', passed=True)]))
        for name, content in files.items():
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text(content)
        self.data = dict(total=3, passed=3, results=results, execution='test external runner',
                         source_sha256={p: hashlib.sha256((self.root / p).read_bytes()).hexdigest()
                                        for p in evidence.source_paths(self.root)})
        self.report = self.root / 'evidence.json'

    def validate(self):
        self.report.write_text(json.dumps(self.data))
        return evidence.validate(self.report, self.root)


class EvidenceTests(EvidenceFixture):
    def test_reviewed_complete_cohort_matches_current_sources(self):
        self.assertIn('3/3 tasks', self.validate())

    def test_failed_skipped_missing_duplicate_and_unclean_tasks_reject(self):
        original = json.dumps(self.data)
        def failed(d): d['results'][0]['verified'] = False
        def skipped(d): d['results'][0]['checks'][0]['skipped'] = True
        def missing(d): d['results'].pop()
        def duplicate(d): d['results'][0] = d['results'][1]
        def unclean(d): d['results'][0]['cleanup_errors'] = ['fixture remains']
        def missing_check(d): d['results'][0]['checks'].pop()
        def status(d): d['results'][0]['agent_status'] = 'error'
        for change in (failed, skipped, missing, duplicate, unclean, missing_check, status):
            with self.subTest(change=change.__name__):
                self.data = json.loads(original)
                change(self.data)
                with self.assertRaises(ValueError): self.validate()

    def test_runtime_skill_harness_and_task_drift_reject(self):
        for name in ('src/main.rs', 'Cargo.lock', 'plugin/skills/computer-pilot/references/scripting.md',
                     'tests/agent/run.py', 'tests/agent/tasks/e2e/mail_to_notes.json'):
            with self.subTest(path=name):
                path = self.root / name
                original = path.read_bytes()
                path.write_bytes(original + b'\n')
                with self.assertRaisesRegex(ValueError, 'stale'): self.validate()
                path.write_bytes(original)

    def test_missing_hash_and_new_runtime_source_reject(self):
        self.data['source_sha256'].pop('src/main.rs')
        with self.assertRaisesRegex(ValueError, 'inventory'): self.validate()
        self.data['source_sha256']['src/main.rs'] = 'irrelevant'
        (self.root / 'src/new.rs').write_text('// new source')
        with self.assertRaisesRegex(ValueError, 'inventory'): self.validate()

    def test_external_report_rejects(self):
        with tempfile.TemporaryDirectory() as other:
            path = Path(other) / 'report.json'
            path.write_text(json.dumps(self.data))
            with self.assertRaises(ValueError): evidence.validate(path, self.root)


class ReleaseRoutingTests(EvidenceFixture):
    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, stderr=subprocess.DEVNULL, text=True).strip()

    def prepare_release_repo(self):
        for name in ('scripts/release.sh', 'scripts/check-agent-evidence.py'):
            p = self.root / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes((ROOT / name).read_bytes())
        for name in ('plugin/.claude-plugin/plugin.json', 'plugin/package.json', '.claude-plugin/marketplace.json'):
            p = self.root / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text('{"version": "0.9.3"}\n')
        # Exercise the real release orchestration; build/desktop checks are
        # stubbed here and covered by the real local verification separately.
        for name in ('check-version-sync.sh', 'verify.sh'):
            (self.root / 'scripts' / name).write_text('#!/bin/sh\nexit 0\n')
        self.validate()
        self.git('init', '-b', 'main')
        self.git('config', 'user.name', 'Test')
        self.git('config', 'user.email', 'test@example.invalid')
        self.git('add', '.')
        self.git('commit', '-m', 'fixture')
        self.remote = self.root.parent / (self.root.name + '-origin')
        self.addCleanup(lambda: shutil.rmtree(self.remote, ignore_errors=True))
        subprocess.run(['git', 'init', '--bare', '-b', 'main', str(self.remote)], check=True, capture_output=True)
        self.git('remote', 'add', 'origin', str(self.remote))
        self.git('push', '-u', 'origin', 'main')
        fakebin = self.root / '.git' / 'fakebin'
        fakebin.mkdir()
        gh = fakebin / 'gh'
        gh.write_text('#!/usr/bin/env python3\nimport json,pathlib,sys\na=sys.argv[1:]\nb=pathlib.Path(a[a.index("--body-file")+1]).read_text()\npathlib.Path(".git/pr.json").write_text(json.dumps(dict(args=a,body=b)))\n')
        gh.chmod(0o700)
        self.env = dict(os.environ, PATH=str(fakebin) + os.pathsep + os.environ['PATH'], OPENAI_API_KEY='test-only-no-api')

    def release(self, *extra):
        return subprocess.run(['bash', 'scripts/release.sh', '0.9.4', '--agent-evidence=evidence.json', *extra],
                              cwd=self.root, env=self.env, capture_output=True, text=True)

    def test_release_uses_evidence_and_records_it_in_pr(self):
        self.prepare_release_repo()
        run = self.release()
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertEqual(self.git('branch', '--show-current'), 'release/v0.9.4')
        self.assertIn('version = "0.9.4"', (self.root / 'Cargo.toml').read_text())
        self.assertEqual(self.git('rev-parse', 'origin/main'), self.git('rev-parse', 'HEAD^'))
        pr = json.loads((self.root / '.git/pr.json').read_text())
        self.assertIn('--draft', pr['args'])
        self.assertIn('test-model', pr['body'])
        self.assertIn('evidence.json', pr['body'])
        self.assertIn('API runner was not invoked', pr['body'])

    def test_bad_evidence_rejects_before_branch_or_version_changes_even_in_dry_run(self):
        self.prepare_release_repo()
        self.data['results'][0]['checks'][0]['passed'] = False
        self.report.write_text(json.dumps(self.data))
        self.git('add', 'evidence.json')
        self.git('commit', '-m', 'failed evidence')
        self.git('push')
        for args in ((), ('--dry-run',)):
            run = self.release(*args)
            self.assertNotEqual(run.returncode, 0)
            self.assertIn('Invalid L2 evidence', run.stderr)
            self.assertEqual(self.git('branch', '--show-current'), 'main')
            self.assertIn('version = "0.9.3"', (self.root / 'Cargo.toml').read_text())
            self.assertFalse((self.root / '.git/pr.json').exists())

    def test_skip_and_evidence_cannot_be_combined(self):
        self.prepare_release_repo()
        run = self.release('--skip-agent')
        self.assertNotEqual(run.returncode, 0)
        self.assertIn('mutually exclusive', run.stderr)
        self.assertEqual(self.git('branch', '--show-current'), 'main')


if __name__ == '__main__':
    unittest.main()
