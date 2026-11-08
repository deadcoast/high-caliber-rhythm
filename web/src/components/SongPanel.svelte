<script>
  import { ed, player, applySongGrid, autoChart, regrid, touch } from '../lib/editor.svelte.js';

  let busy = $state(false);
  let bpm = $state(128);
  let offset = $state(0);

  async function pick(e) {
    const file = e.target.files?.[0];
    if (!file || !ed.core) return;
    busy = true;
    try {
      await player.decode(file);
      ed.song.name = file.name;
      ed.song.duration = player.duration;
      ed.song.loaded = true;
      const a = ed.core.analyze(player.mono(), player.buffer.sampleRate);
      ed.song.analysis = a;
      bpm = Math.round(a.bpm * 100) / 100;
      offset = Math.round(a.offset_s * 1000) / 1000;
      syncGrid();
    } finally {
      busy = false;
    }
  }

  function syncGrid() {
    player.grid.bpm = bpm;
    player.grid.offset = offset;
    regrid(bpm, offset);
  }

  function nudge(ms) {
    const spb = 60 / bpm;
    offset = Math.round((offset + ms / 1000) * 1000) / 1000;
    while (offset < 0) offset += spb;
    while (offset >= spb) offset -= spb;
    offset = Math.round(offset * 1000) / 1000;
    syncGrid();
  }

  function tempo(factor, delta = 0) {
    bpm = Math.round((bpm * factor + delta) * 100) / 100;
    bpm = Math.min(300, Math.max(40, bpm));
    syncGrid();
  }

  function apply() {
    ed.song.analysis = { ...ed.song.analysis, bpm, offset_s: offset };
    applySongGrid();
  }

  function auto() {
    ed.song.analysis = { ...ed.song.analysis, bpm, offset_s: offset };
    applySongGrid();
    autoChart();
  }

  function seekBars(n) {
    const spb = 60 / (ed.level.bpm || bpm);
    const base = ed.level.song_offset_s;
    const k = Math.round((player.position() - base) / (4 * spb)) + n;
    player.seek(Math.max(0, base + k * 4 * spb));
  }

  $effect(() => {
    player.grid.enabled = ed.showClick;
    player.hitsEnabled = ed.showKit;
  });
</script>

<div class="panel col">
  <h2>Song</h2>
  <div class="row">
    <input type="file" accept="audio/*,.wav,.mp3,.ogg,.flac,.m4a" onchange={pick} disabled={!ed.ready || busy} />
    {#if busy}<span class="stat">listening…</span>{/if}
  </div>
  {#if ed.song.analysis}
    <div class="stat">
      {ed.song.name} · {ed.song.duration.toFixed(1)} s · tempo confidence <b>{ed.song.analysis.tempo_confidence?.toFixed(1) ?? '-'}×</b> · phase <b>{ed.song.analysis.phase_confidence?.toFixed(1) ?? '-'}×</b>
    </div>
    <div class="row">
      <label>BPM</label>
      <input type="number" step="0.01" bind:value={bpm} onchange={syncGrid} />
      <button onclick={() => tempo(1, -0.5)}>−0.5</button>
      <button onclick={() => tempo(1, 0.5)}>+0.5</button>
      <button onclick={() => tempo(0.5)}>÷2</button>
      <button onclick={() => tempo(2)}>×2</button>
    </div>
    <div class="row">
      <label>beat 0 at</label>
      <input type="number" step="0.001" bind:value={offset} onchange={syncGrid} />
      <span class="stat">s</span>
      <button onclick={() => nudge(-10)}>−10 ms</button>
      <button onclick={() => nudge(-1)}>−1</button>
      <button onclick={() => nudge(1)}>+1</button>
      <button onclick={() => nudge(10)}>+10 ms</button>
    </div>
    <div class="row">
      <button class="primary" onclick={apply}>use this grid</button>
      <button onclick={auto}>auto chart from energy</button>
    </div>
    <div class="stat">Play with the click on: when the click sits on the kick, the grid is right. The level keeps bpm {ed.level.bpm} and beat 0 at {ed.level.song_offset_s} s.</div>
  {:else}
    <div class="stat">Drop a song to measure its tempo and beat 0. Without one the timeline plays the click and the kit alone.</div>
  {/if}
  <h3>Transport</h3>
  <div class="row">
    <button class="primary" onclick={() => (player.playing ? player.pause() : player.play())}>{ed.playing ? 'pause' : 'play'}</button>
    <button onclick={() => player.stop()}>stop</button>
    <button onclick={() => seekBars(-4)}>−4 bars</button>
    <button onclick={() => seekBars(4)}>+4 bars</button>
    <span class="stat"><b>{ed.position.toFixed(2)}</b> s · beat <b>{((ed.position - ed.level.song_offset_s) / (60 / ed.level.bpm)).toFixed(2)}</b></span>
  </div>
  <div class="row">
    <label><input type="checkbox" bind:checked={ed.showClick} /> click on the grid</label>
    <label><input type="checkbox" bind:checked={ed.showKit} /> kit on every target</label>
  </div>
</div>
