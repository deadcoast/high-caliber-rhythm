<script>
  // The chart over the song: bars ruler, song energy and onsets, segment
  // spans, one lane per enemy kind with lead (thin) and hold (thick) in
  // beats, the strain curve, and the playhead. Click seeks the player.
  import { ed, player } from '../lib/editor.svelte.js';
  import { KIND_COLOR } from '../lib/level.js';

  let canvas = $state();
  let width = $state(900);
  const height = 300;
  let zoom = $state(28); // px per beat
  let scroll = $state(0); // beats at the left edge
  let drag = null;

  const LANES = ['flick', 'switch', 'track', 'micro', 'rush'];
  const laneY = (k) => 110 + LANES.indexOf(k) * 26;

  function spb() {
    return 60 / (ed.level.bpm || 128);
  }
  function songToBeat(t) {
    return (t - ed.level.song_offset_s) / spb();
  }
  const bx = (beat) => (beat - scroll) * zoom;

  function draw() {
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#0b0714';
    ctx.fillRect(0, 0, width, height);
    const chart = ed.chart;
    const a = ed.song.analysis;
    const totalBeats = chart ? chart.total_beats : 64;
    // Song energy per bar and onsets (song time → beat through the level's grid).
    if (a) {
      const sp = spb();
      const rate = a.onset_rate || 20;
      ctx.fillStyle = 'rgba(0,240,255,0.25)';
      for (let i = 0; i < a.onset.length; i++) {
        const beat = songToBeat(i / rate);
        const x = bx(beat);
        if (x < -2 || x > width) continue;
        const h = a.onset[i] * 40;
        ctx.fillRect(x, 95 - h, Math.max(1, zoom / (rate * sp)), h);
      }
      ctx.fillStyle = 'rgba(243,230,0,0.35)';
      const energyBars = a.energy || [];
      for (let b = 0; b < energyBars.length; b++) {
        const x0 = bx(b * 4);
        const x1 = bx(b * 4 + 4);
        if (x1 < 0 || x0 > width) continue;
        const h = energyBars[b] * 40;
        ctx.fillRect(x0 + 1, 95 - h, x1 - x0 - 2, h);
      }
    }
    // Spans
    if (chart) {
      for (const sp of chart.spans) {
        const x0 = bx(sp.start_beat);
        const x1 = bx(sp.end_beat);
        if (x1 < 0 || x0 > width) continue;
        ctx.fillStyle = sp.kind === 'traversal' ? 'rgba(115,51,140,0.35)' : sp.kind === 'boss' ? 'rgba(243,230,0,0.10)' : 'rgba(0,240,255,0.10)';
        ctx.fillRect(x0, 100, x1 - x0, 140);
        ctx.fillStyle = sp.kind === 'boss' ? '#f3e600' : '#8f88a8';
        ctx.font = '10px monospace';
        ctx.fillText(sp.kind === 'traversal' ? 'MOVE' : sp.kind === 'boss' ? `BOSS ${sp.phase + 1} ${sp.label}` : sp.room.toUpperCase(), x0 + 3, 108);
        if (sp.kind === 'boss') {
          ctx.strokeStyle = '#f3e600';
          ctx.beginPath();
          ctx.moveTo(x1, 100);
          ctx.lineTo(x1, 240);
          ctx.stroke();
        }
      }
      // Cues: a mark on the bar with the kind.
      for (const c of chart.cues || []) {
        const x = bx(c.beat);
        if (x < 0 || x > width) continue;
        ctx.fillStyle = '#ffa619';
        ctx.fillRect(x - 1, 14, 2, 10);
        ctx.font = '9px monospace';
        ctx.fillText(c.kind, x + 3, 22);
      }
    }
    // Bars ruler
    const firstBar = Math.floor(scroll / 4);
    const lastBar = Math.ceil((scroll + width / zoom) / 4);
    ctx.font = '10px monospace';
    for (let b = firstBar; b <= lastBar; b++) {
      const x = bx(b * 4);
      ctx.strokeStyle = b % 4 === 0 ? '#3d3560' : '#231d38';
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
      ctx.fillStyle = '#8f88a8';
      ctx.fillText(String(b), x + 2, 10);
      if (zoom >= 18) {
        for (let q = 1; q < 4; q++) {
          const xq = bx(b * 4 + q);
          ctx.strokeStyle = '#1a1528';
          ctx.beginPath();
          ctx.moveTo(xq, 14);
          ctx.lineTo(xq, height);
          ctx.stroke();
        }
      }
    }
    // Lead-in and end
    if (chart) {
      ctx.strokeStyle = '#f3e600';
      ctx.setLineDash([3, 3]);
      ctx.beginPath();
      ctx.moveTo(bx(totalBeats), 0);
      ctx.lineTo(bx(totalBeats), height);
      ctx.stroke();
      ctx.setLineDash([]);
    }
    // Lanes
    for (const k of LANES) {
      const y = laneY(k);
      ctx.fillStyle = KIND_COLOR[k];
      ctx.globalAlpha = 0.6;
      ctx.fillText(k.toUpperCase(), 3, y + 4);
      ctx.globalAlpha = 1;
      ctx.strokeStyle = '#1f1a30';
      ctx.beginPath();
      ctx.moveTo(40, y);
      ctx.lineTo(width, y);
      ctx.stroke();
    }
    if (chart) {
      for (const t of chart.targets) {
        const y = laneY(t.kind);
        const xs = bx(t.spawn_beat);
        const xh = bx(t.hit_beat);
        const xe = bx(t.end_beat);
        if (xe < 0 || xs > width) continue;
        const col = KIND_COLOR[t.kind];
        ctx.strokeStyle = col;
        ctx.globalAlpha = 0.4;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(xs, y);
        ctx.lineTo(xh, y);
        ctx.stroke();
        if (t.kind === 'track') {
          ctx.globalAlpha = 0.8;
          ctx.lineWidth = 6;
          ctx.beginPath();
          ctx.moveTo(xh, y);
          ctx.lineTo(xe, y);
          ctx.stroke();
          ctx.lineWidth = 1;
        }
        ctx.globalAlpha = 1;
        ctx.fillStyle = col;
        const r = Math.max(2.5, Math.min(7, t.radius * 10));
        ctx.beginPath();
        ctx.arc(xh, y, r, 0, Math.PI * 2);
        ctx.fill();
        // Vertical offset within the lane shows yaw: left of centre above, right below.
        ctx.fillStyle = '#e8e3f0';
        ctx.globalAlpha = 0.5;
        ctx.fillRect(xh - 0.5, y + 9, 1, -(t.aim.yaw / 70) * 8);
        ctx.globalAlpha = 1;
      }
      // Strain curve under the lanes
      const strain = chart.rating.strain || [];
      const nonTrack = chart.targets.filter((t) => t.kind !== 'track');
      if (strain.length && strain.length === nonTrack.length) {
        const peak = Math.max(...strain, 1e-6);
        ctx.strokeStyle = '#ff3b3b';
        ctx.globalAlpha = 0.8;
        ctx.beginPath();
        for (let i = 0; i < strain.length; i++) {
          const x = bx(nonTrack[i].hit_beat);
          const y = 290 - (strain[i] / peak) * 40;
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
        ctx.globalAlpha = 1;
        ctx.fillStyle = '#ff3b3b';
        ctx.fillText('strain', 3, 254);
      }
    }
    // Playhead
    const beat = songToBeat(ed.position);
    const px = bx(beat);
    if (px >= 0 && px <= width) {
      ctx.strokeStyle = '#f3e600';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(px, 0);
      ctx.lineTo(px, height);
      ctx.stroke();
      ctx.lineWidth = 1;
    }
  }

  $effect(() => {
    // Any of these changing redraws.
    void ed.chart;
    void ed.position;
    void ed.song.analysis;
    void zoom;
    void scroll;
    void width;
    draw();
  });

  function wheel(e) {
    e.preventDefault();
    const r = canvas.getBoundingClientRect();
    const beatAt = scroll + (e.clientX - r.left) / zoom;
    const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    zoom = Math.min(200, Math.max(4, zoom * factor));
    scroll = Math.max(-8, beatAt - (e.clientX - r.left) / zoom);
  }
  function down(e) {
    drag = { x: e.clientX, scroll, moved: false };
    canvas.setPointerCapture(e.pointerId);
  }
  function move(e) {
    if (!drag) return;
    const dx = e.clientX - drag.x;
    if (Math.abs(dx) > 3) drag.moved = true;
    if (drag.moved) scroll = Math.max(-8, drag.scroll - dx / zoom);
  }
  function up(e) {
    if (drag && !drag.moved) {
      const r = canvas.getBoundingClientRect();
      const beat = scroll + (e.clientX - r.left) / zoom;
      player.seek(ed.level.song_offset_s + beat * spb());
    }
    drag = null;
  }
  function follow() {
    const beat = songToBeat(ed.position);
    if (ed.playing && (beat < scroll || beat > scroll + width / zoom)) scroll = Math.max(-8, beat - 4);
  }
  $effect(() => {
    void ed.position;
    follow();
  });
</script>

<div class="panel">
  <h2>Timeline <span class="stat">beats across · lanes by enemy · lead thin, hold thick · energy per bar (yellow) and onsets (cyan) from the song</span></h2>
  <div bind:clientWidth={width} class="wrap">
    <canvas bind:this={canvas} style="width:{width}px;height:{height}px" onwheel={wheel} onpointerdown={down} onpointermove={move} onpointerup={up}></canvas>
  </div>
</div>

<style>
  .wrap { width: 100%; }
  canvas { cursor: crosshair; border: 1px solid var(--line); border-radius: 3px; }
</style>
