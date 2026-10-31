#!/usr/bin/env python3
"""Coordinator for real-app comparisons; participants use only their assigned app backend."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import plistlib
import signal
import subprocess
import tempfile
import time

import benchmark_varied_workflows as shared

ROOT = shared.ROOT
CU = shared.CU
APPS = {
    'textedit': ('/System/Applications/TextEdit.app', 'TextEdit'),
    'vscode': ('/Applications/Visual Studio Code.app', 'Code'),
    'numbers': ('/Applications/Numbers Creator Studio.app', 'Numbers'),
}


def source(mode):
    if mode == 'textedit':
        text = '\n\n'.join(f'章节 {i} · {title}\n草案：{title}将于 2026-11-15 交付。负责人：{owner}。\n验收：保留中文、标点“样本 {i}”和本段换行。'
                           for i, (title, owner) in enumerate(zip(
                               ['目标', '范围', '交付', '质量', '风险', '结论'],
                               ['林', '陈', '周', '吴', '郑', '王']), 1)) + '\n'
        return dict(mode=mode, files={'brief.txt': text})
    if mode == 'vscode':
        files = {
            'service.json': json.dumps({'environment': 'sandbox', 'endpoint': 'https://sandbox.example.test/api',
                                       'timeoutSeconds': 30, 'debug': True,
                                       'workers': [{'name': n, 'retries': 2} for n in ['ingest', 'index', 'notify']]}, ensure_ascii=False, indent=2) + '\n',
            'release.md': '# sandbox release\n\nDeploy to https://sandbox.example.test/api.\nTimeout: 30 seconds.\nStatus: draft\n\nKeep this note: “中文验收” — build 17.\n',
            'checklist.txt': 'sandbox deployment\n[ ] verify endpoint\n[ ] verify timeout\n[ ] disable debug\n[ ] confirm workers\n',
        }
        return dict(mode=mode, files=files)
    return dict(mode=mode, rows=[['Item', 'Team', 'Quantity', 'Unit price', 'Total'],
        ['Monitor', 'Office', 2, 180, ''], ['Hosting', 'Platform', 12, 25, ''],
        ['Keyboard', 'Office', 4, 45, ''], ['Training', 'People', 3, 120, ''],
        ['Storage', 'Platform', 6, 30, ''], ['Adapter', 'Office', 5, 15, ''],
        ['Grand total', '', '', '', '']])


def verify(spec, saved):
    actual = json.loads(saved.read_text()) if saved.exists() else {}
    checks = {}
    if spec['mode'] == 'textedit':
        expected = spec['files']['brief.txt'].replace('草案', '定稿').replace('2026-11-15', '2026-11-22')
        expected = expected.replace('负责人：周。', '负责人：赵。') + '\n复核完成：6 个章节，保留原始顺序。\n'
        checks['saved_text_exact'] = actual.get('files', {}).get('brief.txt') == expected
    elif spec['mode'] == 'vscode':
        try:
            result = json.loads(actual.get('files', {}).get('service.json', ''))
        except (ValueError, TypeError):
            result = None
        expected = json.loads(spec['files']['service.json'])
        expected.update(environment='production', endpoint='https://production.example.test/api', timeoutSeconds=45, debug=False)
        for worker in expected['workers']:
            worker['retries'] = 4
        checks['config_exact'] = result == expected
        for name in ('release.md', 'checklist.txt'):
            expected_text = spec['files'][name].replace('sandbox', 'production').replace('30 seconds', '45 seconds').replace('Status: draft', 'Status: ready').replace('[ ]', '[x]')
            checks[name + '_exact'] = actual.get('files', {}).get(name) == expected_text
    else:
        rows = [list(row) for row in spec['rows'][1:7]]
        for row in rows:
            if row[0] == 'Hosting': row[2] = 10
            if row[0] == 'Monitor': row[3] = 195
            row[4] = row[2] * row[3]
        rows.sort(key=lambda row: row[0])
        expected = [spec['rows'][0]] + rows + [['Grand total', None, None, None, sum(row[4] for row in rows)]]
        checks['saved_file_exists'] = actual.get('saved_file_exists') is True
        checks['cells_exact'] = actual.get('values') == expected
        checks['formulas_present'] = len(actual.get('formulas', [])) == 7 and all(actual['formulas'])
        checks['sheet_name'] = actual.get('sheet') == 'Budget'
        checks['table_shape'] = (actual.get('row_count'), actual.get('column_count')) == (8, 5)
    return dict(passed=all(checks.values()), checks=checks, actual=actual)


def fingerprints():
    paths = [CU, Path(__file__), ROOT/'tests/agent/benchmark_varied_workflows.py',
             ROOT/'tests/agent/collect_codex_benchmark.py',
             ROOT/'plugin/skills/computer-pilot/SKILL.md',
             ROOT/'plugin/skills/computer-pilot/scripts/session.sh']
    return {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}


def prepare(root):
    root.mkdir(parents=True, exist_ok=False)
    sequence = []
    for i, mode in enumerate(APPS):
        for arm in (('native', 'cu') if i % 2 == 0 else ('cu', 'native')):
            sequence.append(dict(label=f'{mode}-1-{arm}', mode=mode, repeat=1, arm=arm))
    manifest = dict(protocol_version=2, single_turn=True, sequence=sequence,
        settings=dict(model='gpt-6-astra', reasoning_effort='high', service_tier=None),
        fingerprints=fingerprints(), cu_commit=subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip(),
        scope='One matched dataset per real app; six planned attempts, no replacements. TextEdit/VS Code share the user application process with dedicated test windows. Numbers starts separately. Fresh model profiles/cwd and cu runtimes. Normal documented app scripting is allowed through cu tell; native uses actual App APIs. No direct file edits, terminal commands inside apps, automation scripts outside the assigned backend, or task-specific solver. Report routes separately; not a pure AX speed comparison.',
        wall_timeout_seconds=600,
        app_versions={mode:plistlib.loads((Path(path)/'Contents/Info.plist').read_bytes()).get('CFBundleShortVersionString') for mode,(path,_) in APPS.items()})
    (root/'manifest.json').write_text(json.dumps(manifest, indent=2))
    print(json.dumps(manifest))


def process_id(mode):
    path, executable = APPS[mode]
    target = path + '/Contents/MacOS/' + executable
    matches = [int(line.split(None,1)[0]) for line in subprocess.check_output(['ps','-axo','pid=,comm='],text=True).splitlines()
               if len(line.split(None,1)) == 2 and line.split(None,1)[1] == target]
    if len(matches) != 1: raise RuntimeError(f'Expected exactly one {mode} process, found {matches}')
    return matches[0]


def jxa(script):
    return subprocess.check_output(['osascript','-l','JavaScript','-e',script],text=True,timeout=45).strip()


def start(root, label):
    manifest=json.loads((root/'manifest.json').read_text())
    if manifest['fingerprints'] != fingerprints(): raise RuntimeError('Frozen inputs changed')
    entry=next(e for e in manifest['sequence'] if e['label']==label)
    mode=entry['mode'];trial=root/label;trial.mkdir()
    work=trial/('CP-'+root.name+'-'+label);work.mkdir()
    spec=source(mode);(trial/'source.json').write_text(json.dumps(spec,ensure_ascii=False))
    files = {name: (work.name+'-'+name if mode=='textedit' else name) for name in spec.get('files', {})}
    if mode in ('textedit','vscode'):
        for name,text in spec['files'].items():(work/files[name]).write_text(text)
    else:
        with (work/(work.name+'-budget.csv')).open('w',newline='') as f:csv.writer(f).writerows(spec['rows'])
    path,_=APPS[mode]
    if mode=='vscode':
        env=dict(os.environ);env.pop('VSCODE_IPC_HOOK_CLI',None)
        subprocess.run([path+'/Contents/Resources/app/bin/code','--new-window',str(work),str(work/'service.json')],env=env,check=True,timeout=30)
    else:
        file=work/(files['brief.txt'] if mode=='textedit' else work.name+'-budget.csv')
        subprocess.run(['open','-g','-a',path,str(file)],check=True)
    # App launch is outside the task timer. No task edits or strategy coaching.
    pid=None
    for _ in range(40):
        try:pid=process_id(mode);break
        except RuntimeError:time.sleep(0.25)
    if pid is None:raise RuntimeError('App did not start')
    runtime=tempfile.mkdtemp(prefix='cu-real-')
    meta=dict(entry,pid=pid,app_path=path,runtime_home=runtime,client_key=label,
              output_dir=str(trial/'output'),workspace=str(work),
              source_sha256=hashlib.sha256((trial/'source.json').read_bytes()).hexdigest(),
              shared_app=mode!='numbers',files=files,output_file=str(work/(work.name+'-budget.numbers')))
    (trial/'meta.json').write_text(json.dumps(meta,indent=2))
    if mode=='textedit':
        goal=f"In the supplied TextEdit document {work / files['brief.txt']}, read all six sections. Replace every 草案 with 定稿, change every 2026-11-15 to 2026-11-22, change only section 3 owner from 周 to 赵, preserve all other content, punctuation, line breaks and section order. Append one blank line then 复核完成：6 个章节，保留原始顺序。 and a final newline. Save the same plain-text file and observe the final document."
    elif mode=='vscode':
        goal=f"In the supplied VS Code workspace {work}, inspect service.json, release.md and checklist.txt. Update every sandbox environment/URL occurrence to production. In service.json set timeoutSeconds=45, debug=false, and retries=4 for all three workers, preserving other values. In release.md change Timeout to 45 seconds and Status to ready. In checklist.txt mark all four boxes [x]. Preserve unrelated text and line breaks. Save all three files and observe their final contents. Use only the editor UI; no integrated terminal, scripts, extension automation or direct filesystem operations."
    else:
        goal=f"In the supplied Numbers document imported from {work / (work.name+'-budget.csv')}, read the six expense rows. Change Hosting Quantity to 10 and Monitor Unit price to 195. Put a formula multiplying Quantity by Unit price in each Total cell, and a SUM formula in the Grand total Total cell. Sort only the six data rows alphabetically by Item (keep header first and Grand total last). Rename the sheet Budget. Save the native Numbers document at {meta['output_file']} and observe the final table. Keep exactly the existing five columns and eight rows; do not replace formulas with literal results."
    brief=f'''You are a participant in a user-authorized real desktop application benchmark. Use only the dedicated test window/workspace below. The user explicitly permits test-only edits and normal UI dialogs needed to save these local artifacts. Foreground the dedicated test window if necessary for editor input. Keep VS Code Restricted Mode if asked to trust the folder. TextEdit and VS Code share existing app processes; never read/edit other documents or windows, quit the app, change user settings, sign in, install extensions, or send network messages. Numbers has an owned test process. The target window must correspond to the supplied document/workspace; if the UI shows something else, select the test window through the assigned backend or report blocked. Do not inspect benchmark code, source.json, saved.json, other trials or expected results. Do not invoke another model. No coaching is provided.

Task:
{goal}

Initialize and complete this workflow in ONE timed turn, including skill/API discovery. Read source content only through the assigned application backend. Shell writes are allowed only for task-owned configuration, not the documents. Do not use direct document filesystem access, application scripting consoles, external automation or browser/DOM tools to bypass the assigned backend. Ordinary grouping and reasoning from observed content are allowed; no prewritten solvers. Do not add artificial sleeps. Report DONE only after saving and observing the intended final result, or report the concrete blocker. All attempts, errors and recovery costs are retained; an independent coordinator verifies saved artifacts.
'''
    if entry['arm']=='cu':
        brief+=f'''Your backend is the existing shell plus {CU}, guided by {ROOT/'plugin/skills/computer-pilot/SKILL.md'}. Initialize the skill's task config with CU_APP=pid:{pid}, COMPUTER_PILOT_CLIENT_KEY={label}, COMPUTER_PILOT_HOME={runtime}, COMPUTER_PILOT_OUTPUT_DIR={trial/'output'}. Binary/permissions are verified. Both UI commands and cu tell for app-native document data are allowed; do not use do shell script, System Events, file IO or another app inside tell. All scripting must remain on the supplied document. Report which route you used in the final response.'''
    else:
        brief+=f'''Your backend is only the actual mcp__cua_repl native App APIs. Bind {path} with cua.getApp and read its documentation. This explicitly authorizes that backend for this comparison. Use the supplied test document/window only. Do not use cu, shell automation, AppleScript, browser/DOM APIs or another backend. Report which route you used in the final response.'''
    (trial/'brief.txt').write_text(brief)
    print(json.dumps(meta))


def capture_saved(root,label):
    trial=root/label;meta=json.loads((trial/'meta.json').read_text());work=Path(meta['workspace'])
    spec=json.loads((trial/'source.json').read_text())
    if meta['mode']!='numbers':
        result={'files':{name:(work/meta['files'][name]).read_text() if (work/meta['files'][name]).exists() else None for name in spec['files']}}
    else:
        output=Path(meta['output_file']);result={'saved_file_exists':output.exists()}
        if output.exists():
            # Close only the task document without saving, then reopen disk state.
            script='''var a=Application('Numbers');
var names=NAMES;
a.documents().filter(d=>names.includes(d.name())).forEach(d=>d.close({saving:'no'}));
var d=a.open(Path(OUTPUT));var s=d.sheets()[0];var t=s.tables()[0];
var values=[];for(var r=1;r<=8;r++){var row=[];for(var c=0;c<5;c++)row.push(t.cells[String.fromCharCode(65+c)+r].value());values.push(row);}
var formulas=[];for(var r=2;r<=8;r++)formulas.push(t.cells['E'+r].formula());
JSON.stringify({values:values,formulas:formulas,sheet:s.name(),row_count:t.rowCount(),column_count:t.columnCount()});'''.replace('OUTPUT',json.dumps(str(output))).replace('NAMES',json.dumps([output.stem,output.name,output.with_suffix('.csv').name]))
            try:result.update(json.loads(jxa(script)))
            except (subprocess.SubprocessError,ValueError) as e:result['verification_error']=str(e)
    (trial/'saved.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))


def stop(root,label):
    trial=root/label;meta=json.loads((trial/'meta.json').read_text())
    if not meta['pid']:return
    if not meta['shared_app']:
        prefix=json.dumps(Path(meta['workspace']).name)
        jxa("var a=Application('Numbers');a.documents().filter(d=>d.name().startsWith("+prefix+")).forEach(d=>d.close({saving:'no'}));")
        try:os.kill(meta['pid'],signal.SIGTERM)
        except ProcessLookupError:pass
    else:
        # Close only an observed exact test window, never the shared application.
        env=dict(os.environ,COMPUTER_PILOT_HOME=meta['runtime_home'],COMPUTER_PILOT_CLIENT_KEY=meta['client_key'])
        raw=subprocess.run([str(CU),'--json','window','list','--app',f"pid:{meta['pid']}"],env=env,capture_output=True,text=True)
        if raw.returncode==0:
            windows=json.loads(raw.stdout).get('windows',[])
            needle=meta['files']['brief.txt'] if meta['mode']=='textedit' else Path(meta['workspace']).name
            matches=[(i+1,w) for i,w in enumerate(windows) if needle in w.get('title','')]
            if len(matches)==1:
                subprocess.run([str(CU),'--json','window','close','--app',f"pid:{meta['pid']}",'--window',str(matches[0][1]['index'])],env=env,capture_output=True)
    env=dict(os.environ,COMPUTER_PILOT_HOME=meta['runtime_home'],COMPUTER_PILOT_CLIENT_KEY=meta['client_key'])
    status=subprocess.run([str(CU),'--json','status'],env=env,capture_output=True,text=True)
    if status.returncode==0:
        value=json.loads(status.stdout)
        if value.get('running'):
            try:os.kill(value['pid'],signal.SIGTERM)
            except ProcessLookupError:pass
    meta['pid']=None;(trial/'meta.json').write_text(json.dumps(meta,indent=2))


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path)
    p.add_argument('action',choices=['prepare','start','collect','stop','report']);p.add_argument('label',nargs='?')
    p.add_argument('--rollouts',type=Path);args=p.parse_args();root=args.root.resolve()
    if args.action=='prepare':prepare(root)
    elif args.action=='start':start(root,args.label)
    elif args.action=='collect':
        capture_saved(root,args.label);shared.collect(root,args.label,args.rollouts,verify)
    elif args.action=='stop':stop(root,args.label)
    else:shared.report(root)


if __name__=='__main__':main()
