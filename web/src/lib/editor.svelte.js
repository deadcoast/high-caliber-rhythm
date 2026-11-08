// Editor state (Svelte 5 runes) shared by every panel, plus the few actions
// that touch the core. Level edits mutate `ed.level` in place; App recompiles
// on every change.
import { newLevel, normalise, saveLocal, loadLocal } from './level.js';
import { Player } from './audio.js';

export const ed = $state({
  core: null,
  ready: false,
  level: loadLocal() ?? newLevel(),
  builtin: { patterns: [], rooms: [], drills: [] },
  chart: null,
  error: '',
  sim: null,
  simBusy: false,
  song: { name: '', analysis: null, duration: 0, loaded: false },
  selectedPattern: -1,
  selectedEvent: -1,
  selectedSegment: -1,
  position: 0,
  playing: false,
  showClick: true,
  showKit: true,
  version: 0,
});

export const player = new Player();

export function touch() {
  ed.version++;
}

export function allPatterns() {
  const names = new Map();
  for (const p of ed.builtin.patterns) names.set(p.name, p);
  for (const p of ed.level.patterns) names.set(p.name, p);
  return [...names.values()];
}

export function patternByName(name) {
  return ed.level.patterns.find((p) => p.name === name) ?? ed.builtin.patterns.find((p) => p.name === name) ?? null;
}

export function compile() {
  if (!ed.core) return;
  const level = normalise(ed.level);
  const out = ed.core.call({ op: 'compile', level });
  if (out.error) {
    ed.error = out.error;
    ed.chart = null;
  } else {
    ed.error = '';
    ed.chart = out;
    player.hits = out.targets
      .map((t) => ({ time: t.hit_time + level.song_offset_s, kind: t.kind }))
      .sort((a, b) => a.time - b.time);
  }
  saveLocal(ed.level);
}

export function simulate(bot) {
  if (!ed.core || ed.error) return;
  ed.simBusy = true;
  // Let the button repaint before the bot runs (a long level takes ~100 ms).
  setTimeout(() => {
    ed.sim = ed.core.call({ op: 'simulate', level: normalise(ed.level), bot, seed: 7 });
    ed.simBusy = false;
  }, 10);
}

export function applySongGrid() {
  const a = ed.song.analysis;
  if (!a) return;
  ed.level.bpm = Math.round(a.bpm * 100) / 100;
  ed.level.song_offset_s = Math.round(a.offset_s * 1000) / 1000;
  if (ed.song.name) ed.level.audio = 'res://audio/' + ed.song.name;
  if (!ed.level.song) ed.level.song = ed.song.name.replace(/\.[^.]+$/, '');
  touch();
}

export function autoChart() {
  const a = ed.song.analysis;
  if (!a || !ed.core) return;
  const out = ed.core.call({
    op: 'auto_level',
    name: ed.level.name,
    audio: ed.level.audio || 'res://audio/' + ed.song.name,
    bpm: a.bpm,
    offset_s: a.offset_s,
    duration_s: a.duration_s,
    energy: a.energy,
  });
  if (out.level) {
    const keep = { name: ed.level.name, song: ed.level.song, artist: ed.level.artist, charter: ed.level.charter, patterns: ed.level.patterns, rooms: ed.level.rooms, kit: ed.level.kit, stems: ed.level.stems };
    Object.assign(ed.level, out.level, keep);
    ed.level.difficulty = ed.level.difficulty || 'Auto';
    touch();
  }
}

export function regrid(bpm, offset) {
  if (!ed.core || !ed.song.analysis) return;
  const out = ed.core.call({ op: 'regrid', bpm, offset_s: offset });
  if (!out.error) ed.song.analysis = out;
}

export function resetLevel() {
  ed.level = newLevel();
  ed.selectedPattern = -1;
  ed.selectedEvent = -1;
  ed.selectedSegment = -1;
  touch();
}
