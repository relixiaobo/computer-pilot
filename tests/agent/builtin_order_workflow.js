// Run this definition in Codex's actual cua_repl, then call
// runNativeBenchmark(await cua.getApp(session.app_path)). Bind the app in a
// separate call so its initial observation is outside the measured interval.
// Uses only the documented App methods. All copied values come from live AX.
var runNativeBenchmark = async function (app) {
  const timings = [], trace = [], tree = new Map();
  const call = async (method, ...args) => {
    const start = Date.now();
    const result = await app[method](...args);
    timings.push({ method, ms: Date.now() - start });
    trace.push({ method, args, result: typeof result === 'string' ? result : undefined });
    return result;
  };
  const read = async () => {
    const text = await call('getAXState', tree.size
      ? { emit: false } : { emit: false, disableDiffing: true });
    if (!text.startsWith('The following is a diff') && /\n\s*0 standard window/.test(text)) tree.clear();
    const removed = text.match(/Removed element IDs: ([^\n]+)/);
    if (removed) for (const range of removed[1].matchAll(/(\d+)(?:-(\d+))?/g)) {
      for (let id = Number(range[1]); id <= Number(range[2] ?? range[1]); id++) tree.delete(id);
    }
    for (const line of text.split('\n')) {
      const m = line.match(/^[~+]?\s*(\d+) (.+)$/);
      if (m) tree.set(Number(m[1]), m[1] + ' ' + m[2]);
    }
    return [...tree.entries()].sort((a, b) => a[0] - b[0]).map(x => x[1]).join('\n');
  };
  const index = (state, pattern) => {
    const line = state.split('\n').find(l => pattern.test(l));
    if (!line) throw Error('Missing UI target: ' + pattern);
    return Number(line.match(/^\d+/)[0]);
  };
  const field = (s, name) => index(s, new RegExp('text field.*(?:\\) ' + name + '(?:,|$)|ID: ' + name + '(?:,|$))'));
  const button = (s, name) => index(s, new RegExp(' button ' + name + '(?:,|$)'));
  const started = Date.now();
  let state = await read();
  for (let order = 0; order < 6; order++) {
    const m = state.match(/Source Customer: (.*?) Source SKU: (.*?) Source Quantity: (.*?) Source City: (.*?) Source Memo: (.*?) Customer(?:\n|$)/s);
    if (!m) throw Error('Missing source in current UI');
    const values = Object.fromEntries(['Customer', 'SKU', 'Quantity', 'City', 'Memo'].map((k, i) => [k, m[i + 1]]));
    for (const [name, value] of Object.entries(values)) await call('setValue', field(state, name), value);
    if (Number(values.Quantity) >= 10) await call('click', index(state, /checkbox Priority/));
    state = await read();
    await call('click', button(state, 'Save and next'));
    state = await read();
  }
  await call('click', button(state, 'Edit order 2'));
  state = await read();
  await call('setValue', field(state, 'City'), '杭州');
  state = await read();
  await call('click', button(state, 'Save correction'));
  state = await read();
  await call('click', button(state, 'Edit order 5'));
  state = await read();
  await call('setValue', field(state, 'Quantity'), '14');
  await call('click', index(state, /checkbox Priority/));
  state = await read();
  await call('click', button(state, 'Save correction'));
  state = await read();
  await call('click', button(state, 'Finish archive'));
  state = await read();
  const elapsed_ms = Date.now() - started, groups = {};
  for (const t of timings) {
    const g = groups[t.method] ??= { count: 0, ms: 0 };
    g.count++; g.ms += t.ms;
  }
  return { elapsed_ms, api_calls: timings.length, groups, final_state: state, timings, trace };
};
