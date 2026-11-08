<script>
  import { onMount } from 'svelte';
  import { ed, player, compile } from './lib/editor.svelte.js';
  import { loadCore } from './lib/core.js';
  import SongPanel from './components/SongPanel.svelte';
  import LevelPanel from './components/LevelPanel.svelte';
  import PatternPanel from './components/PatternPanel.svelte';
  import Timeline from './components/Timeline.svelte';
  import ReportPanel from './components/ReportPanel.svelte';

  let loadError = $state('');

  onMount(async () => {
    try {
      ed.core = await loadCore(new URL('downbeat_core.wasm', document.baseURI));
      ed.builtin.patterns = ed.core.call({ op: 'patterns' }).patterns;
      ed.builtin.rooms = ed.core.call({ op: 'rooms' }).rooms;
      ed.builtin.drills = ed.core.call({ op: 'drills' }).drills;
      ed.ready = true;
      compile();
    } catch (e) {
      loadError = 'core failed to load: ' + e + '  (run `npm run core` to build and stage downbeat_core.wasm)';
    }
    const tick = () => {
      ed.position = player.position();
      ed.playing = player.playing;
      requestAnimationFrame(tick);
    };
    tick();
  });

  // Recompile on any edit: `version` is bumped by the panels, and the deep
  // read of the level below subscribes to direct field edits too.
  let timer = null;
  $effect(() => {
    if (!ed.ready) return;
    JSON.stringify(ed.level);
    void ed.version;
    clearTimeout(timer);
    timer = setTimeout(compile, 60);
  });

  function keys(e) {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT' || e.target.tagName === 'TEXTAREA') return;
    if (e.code === 'Space') {
      e.preventDefault();
      if (player.playing) player.pause();
      else player.play();
    } else if (e.code === 'Home') {
      player.seek(0);
    }
  }
</script>

<svelte:window onkeydown={keys} />

<div class="app">
  <header>
    <span class="logo">DOWNBEAT</span>
    <span class="sub">chart editor</span>
    {#if ed.chart}
      <span class="stat">bpm <b>{ed.chart.bpm.toFixed(2)}</b></span>
      <span class="stat">targets <b>{ed.chart.targets.length}</b></span>
      <span class="stat">bars <b>{(ed.chart.total_beats / 4).toFixed(1)}</b></span>
      <span class="stat">length <b>{ed.chart.total_time.toFixed(1)} s</b></span>
      <span class="stat">stars <b>{ed.chart.rating.stars.toFixed(2)}</b></span>
      <span class="stat">fingerprint <b>{ed.chart.fingerprint}</b></span>
    {/if}
    {#if ed.error}<span class="err">{ed.error}</span>{/if}
    {#if loadError}<span class="err">{loadError}</span>{/if}
    <span class="spacer"></span>
    <span class="stat">Space play/pause · Home rewind · click the timeline to seek · wheel zoom · drag pan</span>
  </header>
  <main>
    <div class="left col">
      <SongPanel />
      <LevelPanel />
    </div>
    <div class="centre col">
      <Timeline />
      <PatternPanel />
    </div>
    <div class="right col">
      <ReportPanel />
    </div>
  </main>
</div>

<style>
  .app { display: flex; flex-direction: column; height: 100%; }
  header { display: flex; gap: 16px; align-items: baseline; padding: 8px 14px; border-bottom: 1px solid var(--line); background: var(--ink-2); flex-wrap: wrap; }
  .logo { color: var(--yellow); font-weight: 800; letter-spacing: 0.2em; }
  .sub { color: var(--dim); }
  .spacer { flex: 1; }
  main { display: grid; grid-template-columns: 340px 1fr 320px; gap: 10px; padding: 10px; flex: 1; min-height: 0; overflow: auto; }
  .left, .right { min-width: 0; }
  .centre { min-width: 0; }
</style>
