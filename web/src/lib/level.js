// Level document helpers: defaults, normalisation, export, local storage.

export const KINDS = ['flick', 'switch', 'track', 'micro', 'rush'];
export const KIND_COLOR = {
  flick: '#ff2bd6',
  switch: '#ffa619',
  track: '#00f0ff',
  micro: '#a6ff40',
  rush: '#f3e600',
};

export function newLevel() {
  return {
    name: 'untitled',
    song: '',
    artist: '',
    difficulty: 'Normal',
    charter: '',
    bpm: 128,
    lead_in: 8,
    sections: [],
    segments: [
      { type: 'traversal', bars: 1 },
      { type: 'encounter', room: 'range', sections: [{ pattern: 'flick_lr', repeats: 2 }] },
    ],
    rooms: [],
    scaling: { ref_bpm: 128, radius_exp: 0 },
    patterns: [],
    audio: '',
    stems: [],
    song_offset_s: 0,
    kit: {},
    cues: [],
  };
}

export const CUE_KINDS = ['lights', 'train', 'flood', 'reveal', 'shake'];

export function newEvent(beat = 0, kind = 'flick') {
  return { beat, kind, aim: { yaw: 0, pitch: 0, dist: 12 }, radius: 0.45, lead: 1, hold: 0, to: null };
}

export function newPattern(name = 'custom') {
  // Four quarter-note drones, left/right so nothing stacks on the canvas.
  const events = [newEvent(0), newEvent(1), newEvent(2), newEvent(3)];
  events.forEach((e, i) => (e.aim.yaw = i % 2 === 0 ? -22 : 22));
  return { name, length: 4, events };
}

/** The level as the core reads it: numbers coerced, empty optionals dropped. */
export function normalise(level) {
  const l = JSON.parse(JSON.stringify(level));
  l.bpm = Number(l.bpm) || 128;
  l.lead_in = Number(l.lead_in) || 0;
  l.song_offset_s = Number(l.song_offset_s) || 0;
  for (const p of l.patterns) {
    p.length = Number(p.length) || 4;
    for (const e of p.events) {
      e.beat = Number(e.beat) || 0;
      e.radius = Number(e.radius) || 0.3;
      e.lead = Number(e.lead) || 0;
      e.hold = Number(e.hold) || 0;
      e.aim = { yaw: Number(e.aim.yaw) || 0, pitch: Number(e.aim.pitch) || 0, dist: Number(e.aim.dist) || 12 };
      if (e.kind !== 'track') {
        e.hold = 0;
        e.to = null;
      } else if (e.to) {
        e.to = { yaw: Number(e.to.yaw) || 0, pitch: Number(e.to.pitch) || 0, dist: Number(e.to.dist) || 12 };
      }
      if (e.to === null) delete e.to;
    }
  }
  for (const s of l.segments) {
    if (s.type === 'traversal') s.bars = Number(s.bars) || 1;
    else if (s.type === 'boss') {
      for (const ph of s.phases) {
        ph.clear = Math.min(1, Math.max(0, Number(ph.clear) || 0.7));
        for (const sec of ph.sections) sec.repeats = Math.max(1, Math.round(Number(sec.repeats) || 1));
      }
    } else for (const sec of s.sections) sec.repeats = Math.max(1, Math.round(Number(sec.repeats) || 1));
  }
  l.cues = (l.cues || []).map((c) => ({ bar: Number(c.bar) || 1, kind: c.kind || 'lights', room: c.room || '', bars: Number(c.bars) || 0, param: Number(c.param) || 0 }));
  for (const r of l.rooms) {
    r.size = r.size.map(Number);
    r.stand = r.stand.map(Number);
    r.yaw_deg = Number(r.yaw_deg) || 0;
    if (r.origin === null) delete r.origin;
  }
  return l;
}

export function download(name, text) {
  const blob = new Blob([text], { type: 'application/json' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

const KEY = 'downbeat.editor.level';

export function saveLocal(level) {
  try {
    localStorage.setItem(KEY, JSON.stringify(level));
  } catch {
    /* private window or quota: the export button still works */
  }
}

export function loadLocal() {
  try {
    const s = localStorage.getItem(KEY);
    return s ? JSON.parse(s) : null;
  } catch {
    return null;
  }
}
