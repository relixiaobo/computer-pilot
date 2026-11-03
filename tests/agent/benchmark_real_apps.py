#!/usr/bin/env python3
"""Coordinator for real-app comparisons; participants use only their assigned app backend."""
import argparse
import csv
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import plistlib
import random
import signal
import subprocess
import tempfile
import time

import benchmark_varied_workflows as shared

ROOT = shared.ROOT
CU = shared.CU
DEFAULT_MODEL_CALLS = 100
DEFAULT_TOTAL_TOKENS = 8_000_000
APPS = {
    'textedit': ('/System/Applications/TextEdit.app', 'TextEdit'),
    'vscode': ('/Applications/Visual Studio Code.app', 'Code'),
    'numbers': ('/Applications/Numbers Creator Studio.app', 'Numbers'),
    'finder': ('/System/Library/CoreServices/Finder.app', 'Finder'),
}


def source(mode, dataset='baseline', scenario=None):
    if dataset == 'holdout':
        return json.loads((ROOT/'tests/agent/tasks/textedit_holdout.json').read_text())
    if dataset == 'editor':
        return json.loads((ROOT/'tests/agent/tasks/editor_completion.json').read_text())[scenario]
    if dataset == 'varied':
        return json.loads((ROOT/'tests/agent/tasks/editor_varied.json').read_text())[scenario]
    if dataset == 'finder':
        return json.loads((ROOT/'tests/agent/tasks/finder_varied.json').read_text())[scenario]
    if dataset == 'scoped':
        return json.loads((ROOT/'tests/agent/tasks/real_apps_scoped.json').read_text())[mode]
    if mode == 'textedit':
        text = '\n\n'.join(f'章节 {i} · {title}\n草案：{title}将于 2026-11-15 交付。负责人：{owner}。\n验收：保留中文、标点“样本 {i}”和本段换行。'
                           for i, (title, owner) in enumerate(zip(
                               ['目标', '范围', '交付', '质量', '风险', '结论'],
                               ['林', '陈', '周', '吴', '郑', '王']), 1)) + '\n'
        if dataset == 'alternate':
            for old, new in zip(['目标', '范围', '交付', '质量', '风险', '结论'],
                                ['摘要', '指标', '部署', '审计', '依赖', '回滚']):
                text = text.replace('· ' + old, '· ' + new)
            text += '备注：批次 ZX-042；存档 https://archive.example.invalid/v2。\n'
        return dict(mode=mode, files={'brief.txt': text})
    if mode == 'vscode':
        files = {
            'service.json': json.dumps({'environment': 'sandbox', 'endpoint': 'https://sandbox.example.test/api',
                                       'timeoutSeconds': 30, 'debug': True,
                                       'workers': [{'name': n, 'retries': 2} for n in ['ingest', 'index', 'notify']]}, ensure_ascii=False, indent=2) + '\n',
            'release.md': '# sandbox release\n\nDeploy to https://sandbox.example.test/api.\nTimeout: 30 seconds.\nStatus: draft\n\nKeep this note: “中文验收” — build 17.\n',
            'checklist.txt': 'sandbox deployment\n[ ] verify endpoint\n[ ] verify timeout\n[ ] disable debug\n[ ] confirm workers\n',
        }
        if dataset == 'alternate':
            files = {name: text.replace('https://sandbox.example.test/api',
                         'https://sandbox.services.example.invalid/v3/events?tenant=atlas-29')
                         .replace('build 17', 'build 29 / ZX-042')
                     for name, text in files.items()}
            config = json.loads(files['service.json'])
            config.update(region='ap-southeast-2', releaseTag='v8.4.13-rc.2')
            files['service.json'] = json.dumps(config, ensure_ascii=False, indent=2) + '\n'
        return dict(mode=mode, files=files)
    rows = [['Item', 'Team', 'Quantity', 'Unit price', 'Total'],
        ['Monitor', 'Office', 2, 180, ''], ['Hosting', 'Platform', 12, 25, ''],
        ['Keyboard', 'Office', 4, 45, ''], ['Training', 'People', 3, 120, ''],
        ['Storage', 'Platform', 6, 30, ''], ['Adapter', 'Office', 5, 15, ''],
        ['Grand total', '', '', '', '']]
    if dataset == 'alternate':
        for row, quantity, price in zip(rows[1:7], [3, 14, 7, 2, 9, 8], [210, 31, 58, 145, 24, 19]):
            row[2:4] = [quantity, price]
    return dict(mode=mode, rows=rows)


def verify(spec, saved):
    actual = json.loads(saved.read_text()) if saved.exists() else {}
    checks = {}
    if 'expected_files' in spec:
        checks = {name + '_exact': actual.get('files', {}).get(name) == expected
                  for name, expected in spec['expected_files'].items()}
        if spec['mode'] == 'finder':
            checks['file_set_exact'] = set(actual.get('files', {})) == set(spec['expected_files'])
    elif spec['mode'] == 'textedit':
        expected = spec['files']['brief.txt'].replace('草案', '定稿').replace('2026-11-15', '2026-11-22')
        expected = expected.replace('负责人：周。', '负责人：赵。') + '\n复核完成：6 个章节，保留原始顺序。\n'
        checks['saved_text_exact'] = actual.get('files', {}).get('brief.txt') == expected
    elif spec['mode'] == 'vscode':
        try:
            result = json.loads(actual.get('files', {}).get('service.json', ''))
        except (ValueError, TypeError):
            result = None
        expected = json.loads(spec['files']['service.json'])
        expected.update(environment='production', endpoint=expected['endpoint'].replace('sandbox', 'production'), timeoutSeconds=45, debug=False)
        for worker in expected['workers']:
            worker['retries'] = 4
        checks['config_exact'] = result == expected
        for name in ('release.md', 'checklist.txt'):
            expected_text = spec['files'][name].replace('sandbox', 'production').replace('30 seconds', '45 seconds').replace('Status: draft', 'Status: ready').replace('[ ]', '[x]')
            checks[name + '_exact'] = actual.get('files', {}).get(name) == expected_text
    else:
        if 'expected_rows' in spec:
            expected = spec['expected_rows']
            rows = expected[1:7]
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
        # Constant formulas can produce today's totals without fulfilling the
        # requested calculation. The coordinator perturbs the reopened document
        # without saving, independently of the participant's completion claim.
        recalculated = [round((row[2] + 1) * (row[3] + 2) * spec.get('total_multiplier', 1), 2) for row in rows]
        checks['formulas_recalculate'] = actual.get('recalculated_totals') == recalculated + [round(sum(recalculated), 2)]
        checks['sheet_name'] = actual.get('sheet') == spec.get('sheet', 'Budget')
        checks['table_shape'] = (actual.get('row_count'), actual.get('column_count')) == (8, 5)
    return dict(passed=all(checks.values()), checks=checks, actual=actual)


def fingerprints():
    paths = [CU, Path(__file__), ROOT/'tests/agent/tasks/real_apps_scoped.json',
             ROOT/'tests/agent/tasks/textedit_holdout.json',
             ROOT/'tests/agent/tasks/editor_completion.json',
             ROOT/'tests/agent/tasks/editor_varied.json',
             ROOT/'tests/agent/tasks/finder_varied.json',
             ROOT/'tests/agent/benchmark_varied_workflows.py',
             ROOT/'tests/agent/collect_codex_benchmark.py',
             ROOT/'tests/agent/run_codex_comparison.py',
             ROOT/'plugin/skills/computer-pilot/SKILL.md',
             ROOT/'plugin/skills/computer-pilot/scripts/session.sh']
    paths.extend(sorted((ROOT/'plugin/skills/computer-pilot/references').glob('*.md')))
    return {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}


def prepare(root, dataset='baseline', cu_only=False, repeats=1, seed=None,
            max_model_calls=DEFAULT_MODEL_CALLS, max_total_tokens=DEFAULT_TOTAL_TOKENS):
    if max_model_calls < 1 or max_total_tokens < 1:
        raise ValueError('Both model-call and total-token budgets must be positive')
    root.mkdir(parents=True, exist_ok=False)
    sequence = []
    modes = (('finder',) if dataset == 'finder' else ('textedit',) if dataset == 'holdout'
             else ('textedit', 'vscode', 'numbers'))
    for i, mode in enumerate(modes):
        for repeat in range(1, repeats + 1):
            for arm in (('cu',) if cu_only else ('native', 'cu') if i % 2 == 0 else ('cu', 'native')):
                sequence.append(dict(label=f'{mode}-{repeat}-{arm}', mode=mode, repeat=repeat, arm=arm))
    if dataset in ('editor', 'varied', 'finder'):
        cases = ('local', 'scoped', 'multi')
        if dataset == 'varied':
            cases = ('json', 'markdown', 'multi')
        elif dataset == 'finder':
            cases = ('organize', 'rename')
        # The shared collector pairs by mode/repeat; give each case and round its own index.
        rounds = range(1, repeats + 1)
        mode_name = 'finder' if dataset == 'finder' else 'vscode'
        sequence = [dict(label=f'{mode_name}-{case}-r{round_no}-{arm}', mode=mode_name, scenario=case,
                         repeat=(round_no - 1) * len(cases) + index, round=round_no, arm=arm)
                    for round_no in rounds for index, case in enumerate(cases, 1)
                    for arm in (('cu',) if cu_only else ('native', 'cu') if index % 2 else ('cu', 'native'))]
    if seed is not None:
        random.Random(seed).shuffle(sequence)
    manifest = dict(protocol_version=3, single_turn=True, sequence=sequence, dataset=dataset,
        repeats=repeats, order_seed=seed,
        settings=dict(model='gpt-6-astra', reasoning_effort='high', service_tier=None),
        fingerprints=fingerprints(), cu_commit=subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip(),
        scope=('Cu-only retest; historical native runs are not repeated. ' if cu_only else 'Matched native/cu trials. ') + f'Dataset: {dataset}; {len(sequence)} planned attempts, no replacements. TextEdit/VS Code/Finder trials share user application processes with dedicated test windows; any Numbers trials start separately. Fresh model profiles/cwd and cu runtimes, cleared clipboard before every attempt. Finder and holdout tasks are UI-only; other datasets permit documented cu tell where their task allows scripting. Native uses actual App APIs. No direct file edits, terminal commands inside apps, automation scripts outside the assigned backend, or task-specific solver. Report routes separately; changed datasets are not paired speed comparisons.',
        wall_timeout_seconds=None,
        budget=dict(model_calls=max_model_calls, total_tokens=max_total_tokens),
        inactivity_timeout_seconds=1200,
        primary_metrics=['saved_artifact_accuracy', 'model_calls', 'tokens'],
        app_versions={mode:plistlib.loads((Path(APPS[mode][0])/'Contents/Info.plist').read_bytes()).get('CFBundleShortVersionString') for mode in dict.fromkeys(e['mode'] for e in sequence)})
    (root/'manifest.json').write_text(json.dumps(manifest, indent=2))
    print(json.dumps(manifest))


def process_ids(mode):
    path, executable = APPS[mode]
    target = path + '/Contents/MacOS/' + executable
    return [int(line.split(None,1)[0]) for line in subprocess.check_output(['ps','-axo','pid=,comm='],text=True).splitlines()
               if len(line.split(None,1)) == 2 and line.split(None,1)[1] == target]


def process_id(mode):
    matches = process_ids(mode)
    if len(matches) != 1: raise RuntimeError(f'Expected exactly one {mode} process, found {matches}')
    return matches[0]


def jxa(script):
    return subprocess.check_output(['osascript','-l','JavaScript','-'],input=script,text=True,timeout=45).strip()


def isolate_clipboard(trial):
    """Coordinator only: preserve all item types, then remove cross-trial input."""
    saved = json.loads(jxa('''ObjC.import('AppKit');var p=$.NSPasteboard.generalPasteboard;
var before=Number(p.changeCount), items=[], native=p.pasteboardItems;
for(var i=0;i<native.count;i++){var item=Number(native.count)===1?p:native.objectAtIndex(i), types=item.types, row={};
  for(var j=0;j<types.count;j++){var type=types.objectAtIndex(j), data=item.dataForType(type);
    if(data.isNil())throw Error('Cannot preserve clipboard type');
    var encoded=ObjC.unwrap(data.base64EncodedStringWithOptions(0));
    if(typeof encoded!=='string')throw Error('Cannot preserve clipboard type');
    row[ObjC.unwrap(type)]=encoded;}
  items.push(row);}
if(Number(p.changeCount)!==before)throw Error('Clipboard changed during capture');
JSON.stringify({change_count:before,items:items});'''))
    path = trial / 'clipboard.json'
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'w') as f:
        json.dump(saved, f)
    cleared = int(jxa('''ObjC.import('AppKit');var p=$.NSPasteboard.generalPasteboard;
if(Number(p.changeCount)!==COUNT)throw Error('Clipboard changed before isolation');
p.clearContents;'''.replace('COUNT', str(saved['change_count']))))
    saved['cleared_count'] = cleared
    path.write_text(json.dumps(saved))


def restore_clipboard(trial, workspace):
    path = trial / 'clipboard.json'
    if not path.exists():
        return
    saved = json.loads(path.read_text())
    if 'cleared_count' not in saved or saved.get('restore_status'):
        return
    try:
        status = jxa('var saved=' + json.dumps(saved) + ';var root=' +
            json.dumps(str(Path(workspace).resolve())) + ''';
ObjC.import('AppKit');var p=$.NSPasteboard.generalPasteboard, count=Number(p.changeCount);
var files=ObjC.deepUnwrap(p.propertyListForType($('NSFilenamesPboardType')));
var owned=Array.isArray(files)&&files.length&&files.every(f=>f.startsWith(root+'/'));
if(count!==saved.cleared_count&&!owned){'skipped_changed';}else{
  var items=saved.items.length===1?[]:saved.items.map(row=>{var item=$.NSPasteboardItem.alloc.init;
    Object.keys(row).forEach(type=>{var data=$.NSData.alloc.initWithBase64EncodedStringOptions($(row[type]),0);
      if(!item.setDataForType(data,$(type)))throw Error('Clipboard type restore failed');});return item;});
  if(Number(p.changeCount)!==count)throw Error('Clipboard changed before restore');
  p.clearContents;
  if(saved.items.length===1){var row=saved.items[0];p.declareTypesOwner($(Object.keys(row)),null);
    Object.keys(row).forEach(type=>{var data=$.NSData.alloc.initWithBase64EncodedStringOptions($(row[type]),0);
      if(!p.setDataForType(data,$(type)))throw Error('Clipboard type restore failed');});
  }else if(items.length&&!p.writeObjects($(items)))throw Error('Clipboard restore failed');
  'restored';}
''')
    except Exception as error:
        saved['restore_status'] = 'restore_failed'
        saved['restore_error'] = str(error)
        path.write_text(json.dumps(saved))
        raise
    saved['restore_status'] = status
    path.write_text(json.dumps(saved))


def start(root, label):
    manifest=json.loads((root/'manifest.json').read_text())
    if manifest['fingerprints'] != fingerprints(): raise RuntimeError('Frozen inputs changed')
    entry=next(e for e in manifest['sequence'] if e['label']==label)
    mode=entry['mode']
    if mode == 'numbers' and process_ids(mode):
        raise RuntimeError('Numbers is already running; an owned fresh process is required. Do not quit a user session.')
    trial=root/label;trial.mkdir()
    (trial/'output').mkdir()
    work=trial/('CP-'+root.name+'-'+label);work.mkdir()
    spec=source(mode,manifest.get('dataset','baseline'),entry.get('scenario'));(trial/'source.json').write_text(json.dumps(spec,ensure_ascii=False))
    files = {name: (work.name+'-'+name if mode=='textedit' else name) for name in spec.get('files', {})}
    if mode in ('textedit','vscode','finder'):
        for name,text in spec['files'].items():
            path = work / files[name]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
    else:
        with (work/(work.name+'-budget.csv')).open('w',newline='') as f:csv.writer(f).writerows(spec['rows'])
    path,_=APPS[mode]
    if mode=='vscode':
        env=dict(os.environ);env.pop('VSCODE_IPC_HOOK_CLI',None)
        subprocess.run([path+'/Contents/Resources/app/bin/code','--new-window',str(work),str(work/next(iter(files)))],env=env,check=True,timeout=30)
    elif mode == 'finder':
        subprocess.run(['open','-g','-a',path,str(work)],check=True,timeout=30)
    else:
        file=work/(files['brief.txt'] if mode=='textedit' else work.name+'-budget.csv')
        subprocess.run(['open','-g', *(['-F'] if mode == 'numbers' else []), '-a',path,str(file)],check=True)
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
    isolate_clipboard(trial)
    if 'goal' in spec:
        goal=spec['goal'].format(workspace=work, input_file=work/(files.get('brief.txt') or work.name+'-budget.csv'), output_file=meta['output_file'])
    elif mode=='textedit':
        goal=f"In the supplied TextEdit document {work / files['brief.txt']}, read all six sections. Replace every 草案 with 定稿, change every 2026-11-15 to 2026-11-22, change only section 3 owner from 周 to 赵, preserve all other content, punctuation, line breaks and section order. Append one blank line then 复核完成：6 个章节，保留原始顺序。 and a final newline. Save the same plain-text file and observe the final document."
    elif mode=='vscode':
        goal=f"In the supplied VS Code workspace {work}, inspect service.json, release.md and checklist.txt. Update every sandbox environment/URL occurrence to production. In service.json set timeoutSeconds=45, debug=false, and retries=4 for all three workers, preserving other values. In release.md change Timeout to 45 seconds and Status to ready. In checklist.txt mark all four boxes [x]. Preserve unrelated text and line breaks. Save all three files and observe their final contents. Use only the editor UI; no integrated terminal, scripts, extension automation or direct filesystem operations."
    else:
        goal=f"In the supplied Numbers document imported from {work / (work.name+'-budget.csv')}, read the six expense rows. Change Hosting Quantity to 10 and Monitor Unit price to 195. Put a formula multiplying Quantity by Unit price in each Total cell, and a SUM formula in the Grand total Total cell. Sort only the six data rows alphabetically by Item (keep header first and Grand total last). Rename the sheet Budget. Save the native Numbers document at {meta['output_file']} and observe the final table. Keep exactly the existing five columns and eight rows; do not replace formulas with literal results."
    brief=f'''You are a participant in a user-authorized real desktop application benchmark. Use only the dedicated test window/workspace below. The user explicitly permits test-only edits and normal UI dialogs needed to save these local artifacts. Foreground the dedicated test window if necessary for editor input. Keep VS Code Restricted Mode if asked to trust the folder. TextEdit, VS Code and Finder share existing app processes; never read/edit other documents or windows, quit the app, change user settings, sign in, install extensions, or send network messages. Numbers has an owned test process. The target window must correspond to the supplied document/workspace; if the UI shows something else, select the test window through the assigned backend or report blocked. Do not inspect benchmark code, source.json, saved.json, other trials or expected results. Do not invoke another model. No coaching is provided.

Task:
{goal}

Initialize and complete this workflow in ONE timed turn, including skill/API discovery. Read source content only through the assigned application backend. Shell writes are allowed only for task-owned configuration, not the documents. Do not use direct document filesystem access, application scripting consoles, external automation or browser/DOM tools to bypass the assigned backend. Ordinary grouping and reasoning from observed content are allowed; no prewritten solvers. Do not add artificial sleeps. Report DONE only after saving and observing the intended final result, or report the concrete blocker. All attempts, errors and recovery costs are retained; an independent coordinator verifies saved artifacts.
'''
    if entry['arm']=='cu':
        route = ('This is an explicitly UI-only comparison: cu tell and all application scripting are forbidden, including capability probes.'
                 if mode == 'finder' or spec.get('ui_only') else
                 'Both UI commands and cu tell for app-native document data are allowed; do not use do shell script, System Events, file IO or another app inside tell. All scripting must remain on the supplied document.')
        brief+=f'''Your backend is the existing shell plus {CU}, guided by {ROOT/'plugin/skills/computer-pilot/SKILL.md'}. Initialize the skill's task config with CU_APP=pid:{pid}, COMPUTER_PILOT_CLIENT_KEY={label}, COMPUTER_PILOT_HOME={runtime}, COMPUTER_PILOT_OUTPUT_DIR={trial/'output'}. Binary/permissions are verified. {route} Report which route you used in the final response.'''
    else:
        brief+=f'''Your backend is only the actual mcp__cua_repl native App APIs. Bind {path} with cua.getApp and read its documentation. This explicitly authorizes that backend for this comparison. Use the supplied test document/window only. Do not use cu, shell automation, AppleScript, browser/DOM APIs or another backend. Report which route you used in the final response.'''
    if budget := manifest.get('budget'):
        brief = brief.replace('ONE timed turn', 'ONE task turn')
        brief += (f"\nEvaluation prioritizes correct saved results, then model rounds and token consumption, not elapsed time. "
                  f"Your budget is {budget['model_calls']} model responses and {budget['total_tokens']} cumulative total tokens "
                  "(input including cached input, plus output including reasoning). Tool calls within a response are not separate model rounds. "
                  "Discovery and recovery count toward the budget. There is no task wall-time deadline.")
    (trial/'brief.txt').write_text(brief)
    if mode == 'numbers':
        # Even with -F, iWork can restore saved or untitled documents. Refuse contaminated setup before any model runs.
        names = []
        for _ in range(40):
            names = json.loads(jxa("JSON.stringify(Application('Numbers').documents.name())"))
            if file.name in names or file.stem in names: break
            time.sleep(0.25)
        (trial/'startup.json').write_text(json.dumps({'documents': names}, indent=2))
        if len(names) != 1 or names[0] not in (file.name, file.stem):
            raise RuntimeError('Numbers restored unexpected documents; inspect startup.json and clean only owned test artifacts before timing.')
    print(json.dumps(meta))


def saved_file_sample(meta, spec):
    """Coordinator-only byte evidence; never feed this oracle to a participant."""
    files = {}
    for name, expected in spec['expected_files'].items():
        try:
            path = Path(meta['workspace']) / name if meta.get('mode') == 'finder' else Path(meta['workspace']) / meta['files'][name]
            if path.is_symlink():
                files[name] = dict(sha256=None, exact=False)
                continue
            data = path.read_bytes()
            files[name] = dict(sha256=hashlib.sha256(data).hexdigest(),
                               exact=data == expected.encode('utf-8'))
        except FileNotFoundError:
            files[name] = dict(sha256=None, exact=False)
    exact = all(f['exact'] for f in files.values())
    if meta.get('mode') == 'finder':
        exact = exact and set(finder_files(Path(meta['workspace']))) == set(spec['expected_files'])
    return dict(files=files, all_exact=exact)


def finder_files(work):
    # Keep unexpected files and symlinks visible to the oracle. Never follow a
    # link outside the owned fixture or fail collection on an extra binary file.
    return {path.relative_to(work).as_posix():
            None if path.is_symlink() else path.read_bytes().decode('utf-8', errors='replace')
            for path in sorted(work.rglob('*'))
            if path.is_symlink() or (path.is_file() and path.name != '.DS_Store')}


def record_saved_timeline(trial, stopped):
    """Passively sample task files every 250 ms until the runner ends the turn.

    Record transitions plus the last sample. Timestamps bound observations,
    not save instants; exact files do not stop, coach or repair the participant.
    """
    meta = json.loads((trial / 'meta.json').read_text())
    spec = json.loads((trial / 'source.json').read_text())
    previous = None
    with (trial / 'saved-timeline.jsonl').open('x') as output:
        while True:
            final = stopped.is_set()
            started = datetime.now(timezone.utc).isoformat()
            sample = saved_file_sample(meta, spec)
            if sample != previous or final:
                output.write(json.dumps(dict(sample_started_at=started,
                    observed_at=datetime.now(timezone.utc).isoformat(), final=final, **sample)) + '\n')
                output.flush()
            if final:
                return
            previous = sample
            stopped.wait(.25)


def capture_saved(root,label):
    trial=root/label;meta=json.loads((trial/'meta.json').read_text());work=Path(meta['workspace'])
    spec=json.loads((trial/'source.json').read_text())
    if meta['mode'] == 'finder':
        result={'files': finder_files(work)}
    elif meta['mode']!='numbers':
        result={'files':{name:(work/meta['files'][name]).read_bytes().decode('utf-8') if (work/meta['files'][name]).exists() else None for name in spec['files']}}
    else:
        output=Path(meta['output_file']);result={'saved_file_exists':output.exists()}
        if output.exists():
            # Close only the task document without saving, then reopen disk state.
            script='''var a=Application('Numbers');
var names=NAMES;
a.documents().filter(d=>names.includes(d.name())).map(d=>d.id()).forEach(id=>a.documents.byId(id).close({saving:'no'}));
var d=a.open(Path(OUTPUT));var s=d.sheets()[0];var t=s.tables()[0];
var values=[];for(var r=1;r<=8;r++){var row=[];for(var c=0;c<5;c++)row.push(t.cells[String.fromCharCode(65+c)+r].value());values.push(row);}
var formulas=[];for(var r=2;r<=8;r++)formulas.push(t.cells['E'+r].formula());
var result={values:values,formulas:formulas,sheet:s.name(),row_count:t.rowCount(),column_count:t.columnCount()};
try {
  for(var r=2;r<=7;r++){t.cells['C'+r].value=values[r-1][2]+1;t.cells['D'+r].value=values[r-1][3]+2;}
  result.recalculated_totals=[];for(var r=2;r<=8;r++)result.recalculated_totals.push(t.cells['E'+r].value());
} finally {d.close({saving:'no'});}
JSON.stringify(result);'''.replace('OUTPUT',json.dumps(str(output))).replace('NAMES',json.dumps([output.stem,output.name,output.with_suffix('.csv').name]))
            try:result.update(json.loads(jxa(script)))
            except (subprocess.SubprocessError,ValueError) as e:result['verification_error']=str(e)
    (trial/'saved.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))


def stop(root,label):
    trial=root/label;meta=json.loads((trial/'meta.json').read_text())
    if not meta['pid']:return
    env=dict(os.environ,COMPUTER_PILOT_HOME=meta['runtime_home'],COMPUTER_PILOT_CLIENT_KEY=meta['client_key'])
    cleanup_error = None
    restore_error = None
    try:
        try:
            if meta['mode'] == 'finder':
                # Finder changes the title on every navigation. Use the actual
                # target URL, with a path boundary, to close only owned windows.
                uri = Path(meta['workspace']).resolve().as_uri() + '/'
                jxa('''var a=Application('Finder');var root=ROOT;
function owned(){return a.windows().filter(w=>{var u=w.target.url();return u===root.slice(0,-1)||u.startsWith(root);});}
owned().map(w=>w.id()).forEach(id=>a.windows.byId(id).close());
if(owned().length)throw Error('Test Finder window remains open');'''.replace('ROOT', json.dumps(uri)))
            elif not meta['shared_app']:
                prefix=json.dumps(Path(meta['workspace']).name)
                jxa("var a=Application('Numbers');var prefix="+prefix+";"
                    "a.documents().filter(d=>d.name().startsWith(prefix)||d.name().startsWith('Untitled ('+prefix)"
                    "||(d.file()&&String(d.file()).split('/').pop().startsWith(prefix)))"
                    ".map(d=>d.id()).forEach(id=>a.documents.byId(id).close({saving:'no'}));"
                    "if(a.documents().length)throw Error('Non-task Numbers documents remain; do not quit');"
                    "a.quit();")
            else:
                # A successful close dispatch can leave an unsaved-changes dialog.
                # Keep ownership recorded until the dedicated window actually closes.
                needle=meta['files']['brief.txt'] if meta['mode']=='textedit' else Path(meta['workspace']).name
                def task_windows():
                    raw=subprocess.run([str(CU),'--json','window','list','--app',f"pid:{meta['pid']}"],env=env,capture_output=True,text=True,check=True)
                    return [w for w in json.loads(raw.stdout)['windows'] if needle in w.get('title','')]
                matches=task_windows()
                if len(matches)==1:
                    subprocess.run([str(CU),'--json','window','close','--app',f"pid:{meta['pid']}",'--window',str(matches[0]['index'])],env=env,capture_output=True,check=True)
                    matches=task_windows()
                    deadline=time.monotonic()+2
                    while matches and time.monotonic()<deadline:
                        time.sleep(0.1)
                        matches=task_windows()
                if matches:
                    raise RuntimeError('Test window remains open; inspect its save dialog before retrying cleanup. Do not quit the shared app.')
        except BaseException as error:
            cleanup_error = error
    finally:
        try:
            restore_clipboard(trial, meta['workspace'])
        except BaseException as error:
            restore_error = error
        status=subprocess.run([str(CU),'--json','status'],env=env,capture_output=True,text=True)
        if status.returncode==0:
            value=json.loads(status.stdout)
            if value.get('running'):
                try:os.kill(value['pid'],signal.SIGTERM)
                except ProcessLookupError:pass
    if cleanup_error is not None:
        if restore_error is not None:
            cleanup_error.add_note(f'Clipboard restoration failed: {restore_error}')
        raise cleanup_error
    if restore_error is not None:
        raise restore_error
    meta['pid']=None;(trial/'meta.json').write_text(json.dumps(meta,indent=2))


def accuracy_summary(attempts):
    accepted = [row for row in attempts if row['status'] in ('passed', 'failed')]
    reviewed = [row for row in accepted if type(row.get('audit', {}).get('claimed_complete')) is bool]
    claims = [row for row in reviewed if row['audit']['claimed_complete']]
    false_claims = sum(row['status'] == 'failed' for row in claims)
    passed = sum(row['status'] == 'passed' for row in attempts)
    return dict(planned_attempts=len(attempts), accepted_completed=len(accepted),
                passed=passed, success_rate=passed / len(attempts) if attempts else None,
                completion_claims_reviewed=len(reviewed), claimed_complete=len(claims),
                false_completion_claims=false_claims,
                false_completion_rate=false_claims / len(claims) if claims else None)


def timeline_metrics(trial):
    """Summarize passive save observations and calls made after the first exact save."""
    timeline_path = trial / 'saved-timeline.jsonl'
    if not timeline_path.exists():
        return {}
    timeline = [json.loads(line) for line in timeline_path.read_text().splitlines()]
    exact_index = next((i for i, sample in enumerate(timeline) if sample.get('all_exact')), None)
    metrics = {'save_observation_count': len(timeline), 'first_exact_index': exact_index}
    if exact_index is None:
        metrics['post_save_tool_calls'] = None
        metrics['post_save_tool_errors'] = None
        return metrics
    first_exact = datetime.fromisoformat(timeline[exact_index]['observed_at']).timestamp() * 1000
    trace_path = trial / 'trace.json'
    calls = []
    if trace_path.exists():
        for turn in json.loads(trace_path.read_text()):
            calls.extend(turn.get('tools', []))
    after = [call for call in calls if call.get('end_ms', 0) > first_exact]
    metrics['post_save_tool_calls'] = len(after)
    metrics['post_save_tool_errors'] = sum(call.get('status') != 'completed' or call.get('exit_code', 0) not in (0, None)
                                          for call in after)
    return metrics


def report(root):
    shared.report(root)
    path = root / 'report.json'
    result = json.loads(path.read_text())
    for attempt in result['attempts']:
        trial = root / attempt['label']
        attempt['timeline'] = timeline_metrics(trial)
    result['accuracy'] = accuracy_summary(result['attempts'])
    path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    if result['manifest'].get('budget'):
        (root/'evidence.md').write_text((root/'report.md').read_text() +
            '\nPassive save transitions and post-save calls remain in report.json; they do not coach or stop the participant.\n')
        print(json.dumps(result['accuracy']))
        return
    lines = [
        '# Varied application execution evidence',
        '',
        'Passive save sampling records when the independently checked files first became exact. '
        'Post-save calls are tool calls whose recorded completion was after that observation; they are '
        'diagnostic and do not alter the pass/fail result.',
        '',
        '| Attempt | Status | Full task s | Calls | Errors | First exact sample | Post-save calls |',
        '| --- | --- | ---: | ---: | ---: | ---: | ---: |',
    ]
    for attempt in result['attempts']:
        timeline = attempt.get('timeline', {})
        runs = attempt.get('runs') or [{}]
        first = timeline.get('first_exact_index')
        first = '—' if first is None else str(first)
        lines.append('| {label} | {status} | {seconds:.1f} | {calls} | {errors} | {first} | {post} |'.format(
            label=attempt['label'], status=attempt['status'],
            seconds=attempt.get('combined_turn_ms', 0) / 1000,
            calls=runs[0].get('tool_calls', '—'),
            errors=runs[0].get('tool_errors', '—'),
            first=first, post=timeline.get('post_save_tool_calls', '—')))
    (root / 'evidence.md').write_text('\n'.join(lines) + '\n')
    print(json.dumps(result['accuracy']))


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path)
    p.add_argument('action',choices=['prepare','start','collect','stop','report']);p.add_argument('label',nargs='?')
    p.add_argument('--rollouts',type=Path)
    p.add_argument('--dataset',choices=['baseline','alternate','scoped','editor','varied','finder','holdout'],default='baseline')
    p.add_argument('--cu-only',action='store_true')
    p.add_argument('--repeats',type=int,default=1)
    p.add_argument('--seed',type=int)
    p.add_argument('--max-model-calls',type=int,default=DEFAULT_MODEL_CALLS,
                   help='Model-response budget per attempt (default: %(default)s)')
    p.add_argument('--max-total-tokens',type=int,default=DEFAULT_TOTAL_TOKENS,
                   help='Cumulative token budget per attempt (default: %(default)s)')
    args=p.parse_args();root=args.root.resolve()
    if args.repeats < 1:p.error('--repeats must be positive')
    if args.action=='prepare':prepare(root,args.dataset,args.cu_only,args.repeats,args.seed,
                                      args.max_model_calls,args.max_total_tokens)
    elif args.action=='start':start(root,args.label)
    elif args.action=='collect':
        capture_saved(root,args.label);shared.collect(root,args.label,args.rollouts,verify)
    elif args.action=='stop':stop(root,args.label)
    else:report(root)


if __name__=='__main__':main()
