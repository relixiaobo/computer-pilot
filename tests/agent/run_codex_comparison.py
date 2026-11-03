#!/usr/bin/env python3
"""Serial model runner; task strategy stays entirely in the participant."""
import argparse, hashlib, json, os, shutil, signal, subprocess, sys, threading, tempfile, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from collect_codex_benchmark import UsageReader


def stop_participant(process):
    try:
        os.killpg(process.pid, signal.SIGINT)
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=15)


def run_participant(process, brief, profile, trial, manifest):
    """Resource limits score tasks; inactivity only diagnoses a stalled runner."""
    process.stdin.write(brief)
    process.stdin.close()
    budget = manifest.get('budget')
    reader = None
    started = last_activity = time.monotonic()
    previous_sizes = None
    while True:
        try:
            process.wait(timeout=1)
            return
        except subprocess.TimeoutExpired:
            pass
        now = time.monotonic()
        stop = None
        if budget:
            if reader is None:
                rollout = next((profile/'sessions').rglob('*.jsonl'), None)
                if rollout is not None:
                    reader = UsageReader(rollout)
            usage = reader.read() if reader else dict(model_calls=0, tokens={})
            exceeded = [name for name, value in (
                ('model_calls', usage['model_calls']),
                ('total_tokens', usage['tokens'].get('total_tokens', 0))) if value >= budget[name]]
            sizes = ((trial/'events.jsonl').stat().st_size,
                     reader.path.stat().st_size if reader else 0)
            if sizes != previous_sizes:
                previous_sizes, last_activity = sizes, now
            if exceeded and not (reader and reader.completed):
                stop = dict(kind='budget_exhausted', limits=budget, exceeded=exceeded, observed=usage)
            elif now - last_activity >= manifest['inactivity_timeout_seconds']:
                stop = dict(kind='infrastructure_stall', observed=usage,
                            inactivity_timeout_seconds=manifest['inactivity_timeout_seconds'])
        elif now - started >= manifest['wall_timeout_seconds']:
            stop = dict(kind='wall_timeout', wall_timeout_seconds=manifest['wall_timeout_seconds'])
            (trial/'timeout.json').write_text(json.dumps(stop))
        if stop:
            (trial/'stop.json').write_text(json.dumps(stop, indent=2))
            stop_participant(process)
            return


def main():
    parser=argparse.ArgumentParser(description="Run prepared real-app trials serially in fresh Codex profiles.")
    parser.add_argument('root',type=Path)
    parser.add_argument('--base-profile',type=Path,required=True,help="Existing authenticated profile with native CUA configured; credentials are not printed or committed")
    parser.add_argument('--from-label',help='Resume at an unstarted manifest label')
    parser.add_argument('--codex',default=shutil.which('codex'))
    args=parser.parse_args()
    repo=Path(__file__).resolve().parents[2]
    root=args.root.resolve()
    base=args.base_profile.resolve()
    neutral=Path(tempfile.mkdtemp(prefix='cu-comparison-model-'))
    harness=repo/'tests/agent/benchmark_real_apps.py'
    def run(*args):
        return subprocess.run([sys.executable,str(harness),str(root),*args],cwd=repo,check=True)
    sys.path.insert(0,str(harness.parent))
    import benchmark_real_apps as bench
    manifest=json.loads((root/'manifest.json').read_text())
    sequence=manifest['sequence']
    if args.from_label:
        sequence=sequence[next(i for i,e in enumerate(sequence) if e['label']==args.from_label):]
    for entry in sequence:
        label=entry['label']; trial=root/label; work=neutral/label; work.mkdir()
        if trial.exists():
            raise FileExistsError(f'Attempt already exists: {trial}; resume at an unstarted label')
        profile=work/'profile'; profile.mkdir(mode=0o700)
        for name in ['auth.json','models_cache.json']:
            (profile/name).symlink_to(base/name)
        (profile/'skills').mkdir()
        shutil.copytree(base/'.tmp/bundled-marketplaces',profile/'.tmp/bundled-marketplaces')
        shutil.copytree(base/'plugins/cache/openai-bundled',profile/'plugins/cache/openai-bundled')
        (profile/'config.toml').write_text((base/'config.toml').read_text().replace(str(base),str(profile)))
        (profile/'config.toml').chmod(0o600)
        cwd=work/'work'; cwd.mkdir()
        try:
            run('start',label)
            env=os.environ.copy(); env['CODEX_HOME']=str(profile)
            for key in ('CODEX_SESSION_ID','CODEX_THREAD_ID','CODEX_TOOL_CALL_ID','COMPUTER_PILOT_HOME','COMPUTER_PILOT_CLIENT_KEY','COMPUTER_PILOT_OUTPUT_DIR','CU_APP'):
                env.pop(key,None)
            cmd=[args.codex,'--no-daemon','exec','-C',str(cwd),'--skip-git-repo-check','-m',manifest['settings']['model'],'-c',f'model_reasoning_effort="{manifest["settings"]["reasoning_effort"]}"','-c','project_doc_max_bytes=0','-c','features.memories=false','-c','notify=[]','-c','approval_policy="never"','-s','danger-full-access','--json','--color','never','-o',str(trial/'final.txt'),'-']
            (trial/'launch.json').write_text(json.dumps({'argv':cmd,'cwd':str(cwd),'profile':str(profile)},indent=2))
            print('START',label,flush=True)
            stopped=threading.Event()
            with ThreadPoolExecutor(max_workers=1) as recorder:
                future=recorder.submit(bench.record_saved_timeline,trial,stopped)
                p=None
                try:
                    with (trial/'events.jsonl').open('w') as out,(trial/'stderr.log').open('w') as err:
                        p=subprocess.Popen(cmd,env=env,cwd=cwd,stdin=subprocess.PIPE,stdout=out,stderr=err,text=True,start_new_session=True)
                        (trial/'process.txt').write_text(str(p.pid))
                        try:
                            run_participant(p, (trial/'brief.txt').read_text(), profile, trial, manifest)
                        finally:
                            if p.poll() is None:
                                stop_participant(p)
                finally:
                    stopped.set(); future.result()
            events=[json.loads(l) for l in (trial/'events.jsonl').read_text().splitlines()]
            tid=next(x['thread_id'] for x in events if x['type']=='thread.started')
            (trial/'thread.txt').write_text(tid+'\n')
            rollout=next((profile/'sessions').rglob('*'+tid+'.jsonl'))
            contexts=[];messages=[]
            for line in rollout.read_text().splitlines():
                e=json.loads(line); v=e.get('payload',{})
                if e['type']=='turn_context': contexts.append({k:v.get(k) for k in ('cwd','model','effort','service_tier')})
                if e['type']=='response_item' and v.get('type')=='message' and v.get('role') in ('user','developer'):
                    s='\n'.join(b.get('text','') for b in v.get('content',[]))
                    messages.append({'role':v['role'],'chars':len(s),'sha256':hashlib.sha256(s.encode()).hexdigest(),'agents_injected':'# AGENTS.md instructions' in s or '# Working preferences' in s or '# Computer Pilot — AGENTS.md' in s})
            audit={'contexts':contexts,'messages':messages,'no_agents_injected':not any(m['agents_injected'] for m in messages),'exit_code':p.returncode}
            (trial/'runtime-context.json').write_text(json.dumps(audit,indent=2))
            run('collect',label,'--rollouts',str(profile/'sessions'))
        finally:
            if (trial/'meta.json').exists():
                run('stop',label)
        print('DONE',label,'exit',p.returncode,'clean_context',audit['no_agents_injected'],flush=True)
        if p.returncode or not audit['no_agents_injected']:
            print('STOP: runtime failure requires review',flush=True); break


if __name__ == "__main__":
    main()
