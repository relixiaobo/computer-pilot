#!/usr/bin/env python3
"""Native dictionary regression on a fresh, owned Numbers trial prepared by benchmark_real_apps.

Run before a model uses that trial; this mutates its disposable table without saving.
The coordinator owns app setup/isolation and cleanup, exactly as for real-app trials.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

import benchmark_real_apps as bench


def check(trial):
    meta = json.loads((trial / 'meta.json').read_text())
    spec = json.loads((trial / 'source.json').read_text())
    assert meta['mode'] == 'numbers' and not meta['shared_app'] and meta['pid']
    env = dict(os.environ, COMPUTER_PILOT_HOME=meta['runtime_home'],
               COMPUTER_PILOT_CLIENT_KEY=meta['client_key'])

    def cu(*args):
        result = subprocess.run([str(bench.CU), '--json', *args], env=env,
                                capture_output=True, text=True, check=True, timeout=20)
        return json.loads(result.stdout)

    dictionary = cu('sdef', 'Numbers')
    sort = next(c for s in dictionary['suites'] for c in s['commands'] if c['name'] == 'sort')
    formula = next(p for s in dictionary['suites'] for c in s['classes']
                   if c['name'] == 'cell' for p in c['properties'] if p['name'] == 'formula')
    assert formula['access'] == 'r' and 'use the value property' in formula['description']
    document = Path(meta['output_file']).stem
    expression = f'''tell document {json.dumps(document)}
tell {sort['direct_parameter']} 1 of sheet 1
sort by {sort['parameter_types']['by']} "A" direction ascending in rows {sort['parameter_types']['in rows']} "A2:E7"
set value of cell "E2" to "=ROUND(C2*D2*0.9,2)"
end tell
end tell'''
    receipt = cu('tell', meta['app_path'], expression)
    # Independently read the changed application state and perturb the formula input.
    actual = json.loads(bench.jxa('var d=Application("Numbers").documents.byName(' +
        json.dumps(document) + ');var t=d.sheets()[0].tables()[0];'
        'var r={names:[],total:t.cells["E2"].value(),formula:t.cells["E2"].formula()};'
        'for(var n=2;n<=7;n++)r.names.push(t.cells["A"+n].value());'
        't.cells["C2"].value=t.cells["C2"].value()+1;'
        'r.recalculated=t.cells["E2"].value();JSON.stringify(r);'))
    ordered = sorted(spec['rows'][1:7], key=lambda row: row[0])
    first = ordered[0]
    assert actual['names'] == [row[0] for row in ordered], actual
    assert actual['total'] == round(first[2] * first[3] * .9, 2), actual
    assert actual['recalculated'] == round((first[2] + 1) * first[3] * .9, 2), actual
    assert actual['formula'], actual
    result = dict(passed=True, sort=sort, formula_property=formula,
                  expression=expression, receipt=receipt, actual=actual)
    (trial / 'dictionary-check.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    check(Path(sys.argv[1]).resolve())
