// Exercise the staged wasm core from Node: every op the editor uses.
// Exit 1 on any mismatch, so `npm run check` is a test.
import fs from 'node:fs';
import { loadCore } from '../src/lib/core.js';
import { normalise } from '../src/lib/level.js';

const bytes = fs.readFileSync(new URL('../public/downbeat_core.wasm', import.meta.url));
const core = await loadCore(bytes);
let fails = 0;
const check = (name, ok, detail = '') => {
  console.log(`  ${ok ? 'ok   ' : 'FAIL '} ${name}${ok ? '' : '   ' + detail}`);
  if (!ok) fails++;
};
const v = core.call({ op: 'version' });
check('version', Array.isArray(v.ops) && v.ops.includes('compile'));
const pats = core.call({ op: 'patterns' }).patterns;
check('patterns', pats.length >= 10 && pats.some((p) => p.name === 'flick_lr'));
check('rooms', core.call({ op: 'rooms' }).rooms.length === 7);
check('drills', core.call({ op: 'drills' }).drills.length >= 7);
const level = { name: 'check', bpm: 140, lead_in: 8, sections: [{ pattern: 'flick_lr', repeats: 4 }, { pattern: 'track_sweep', repeats: 2 }] };
const c = core.call({ op: 'compile', level });
check('compile', !c.error && c.targets.length === 18 && c.fingerprint.length === 16, c.error || `${c.targets?.length}`);
check('compile reports rating', c.rating && c.rating.stars > 0);
check('compile error surfaces', typeof core.call({ op: 'compile', level: { bpm: 'x' } }).error === 'string');
const sim = core.call({ op: 'simulate', level, bot: { perfect: true } });
check('simulate perfect bot', sim.report && sim.report.expired === 0 && sim.report.grade === 'S', JSON.stringify(sim.report?.grade));
const sr = 44100, seconds = 20, n = sr * seconds, f = new Float32Array(n);
for (let b = 0; 0.3 + (b * 60) / 128 < seconds; b++) {
  const s0 = Math.round((0.3 + (b * 60) / 128) * sr);
  for (let i = 0; i < 400 && s0 + i < n; i++) f[s0 + i] = Math.sin(i * 0.3) * (1 - i / 400) * (b % 4 === 0 ? 1.0 : 0.6);
}
const a = core.analyze(f, sr);
check('analyze tempo', Math.abs(a.bpm - 128) < 0.5, `${a.bpm}`);
check('analyze offset', Math.abs(a.offset_s - 0.3) < 0.01, `${a.offset_s}`);
check('analyze onset track', a.onset.length === Math.round(seconds * a.onset_rate) || Math.abs(a.onset.length - seconds * a.onset_rate) <= 2, `${a.onset.length}`);
const re = core.call({ op: 'regrid', bpm: 128, offset_s: 0.3 });
check('regrid', Array.isArray(re.energy) && re.bars === a.bars);
const al = core.call({ op: 'auto_level', name: 'song', audio: 'song.mp3', bpm: a.bpm, offset_s: a.offset_s, duration_s: a.duration_s, energy: a.energy }).level;
check('auto level compiles', !core.call({ op: 'compile', level: al }).error);
// The shipped hand chart goes through the editor's normalise and compiles unchanged.
const night = JSON.parse(fs.readFileSync(new URL('../../godot/levels/night_market_132.json', import.meta.url), 'utf8'));
const nc = core.call({ op: 'compile', level: normalise(night) });
const raw = core.call({ op: 'compile', level: night });
check('night market compiles through the editor', !nc.error && nc.targets.length > 300, nc.error);
check('editor normalise keeps the fingerprint', nc.fingerprint === raw.fingerprint, `${nc.fingerprint} vs ${raw.fingerprint}`);
// Skyline: a boss segment with three phases and cues, through the editor's normaliser.
const sky = JSON.parse(fs.readFileSync(new URL('../../godot/levels/skyline_168.json', import.meta.url), 'utf8'));
const sc = core.call({ op: 'compile', level: normalise(sky) });
check('skyline compiles with its boss', !sc.error && sc.phases?.length === 3 && sc.spans.filter((s) => s.kind === 'boss').length === 3, sc.error);
check('skyline cues compile', Array.isArray(sc.cues) && sc.cues.length === 6);
check('editor normalise keeps skyline fingerprint', sc.fingerprint === core.call({ op: 'compile', level: sky }).fingerprint);
console.log(`${fails ? 'FAIL' : 'PASS'}`);
process.exit(fails ? 1 : 0);
