<script>
  // Inline patterns: the level's own 4-beat phrases. The aim canvas is the
  // player's view from the stand (yaw across, pitch up); drag a weak point to
  // place it, click empty space to add one at the chosen beat. Builtin
  // patterns are read-only until duplicated.
  import { ed, touch, patternByName } from '../lib/editor.svelte.js';
  import { KINDS, KIND_COLOR, newEvent, newPattern } from '../lib/level.js';

  const W = 560;
  const H = 260;
  const YAW = 70; // degrees shown each side
  const PITCH = 32;

  let newBeat = $state(0);
  let newKind = $state('flick');
  let dragging = $state(-1);
  let dragTo = $state(false);
  let dragMoved = false;
  let svg = $state();

  const pattern = $derived(ed.selectedPattern >= 0 ? ed.level.patterns[ed.selectedPattern] : null);

  function select(i) {
    ed.selectedPattern = i;
    ed.selectedEvent = -1;
  }
  function add() {
    ed.level.patterns.push(newPattern('pattern' + (ed.level.patterns.length + 1)));
    select(ed.level.patterns.length - 1);
    touch();
  }
  function duplicate(name) {
    const src = patternByName(name);
    if (!src) return;
    const copy = JSON.parse(JSON.stringify(src));
    copy.name = name + '_' + (ed.level.patterns.length + 1);
    ed.level.patterns.push(copy);
    select(ed.level.patterns.length - 1);
    touch();
  }
  function remove(i) {
    const name = ed.level.patterns[i].name;
    ed.level.patterns.splice(i, 1);
    for (const s of ed.level.segments) if (s.type === 'encounter') for (const sec of s.sections) if (sec.pattern === name) sec.pattern = 'rest';
    select(-1);
    touch();
  }
  function addEvent(beat = newBeat, kind = newKind, yaw = 0, pitch = 0) {
    if (!pattern) return;
    const e = newEvent(Number(beat), kind);
    e.aim.yaw = yaw;
    e.aim.pitch = pitch;
    if (kind === 'track') {
      e.hold = 1;
      e.to = { yaw: yaw + 20, pitch, dist: 12 };
    }
    if (kind === 'micro') e.radius = 0.3;
    pattern.events.push(e);
    pattern.events.sort((a, b) => a.beat - b.beat);
    ed.selectedEvent = pattern.events.indexOf(e);
    touch();
  }
  function removeEvent(k) {
    pattern.events.splice(k, 1);
    ed.selectedEvent = -1;
    touch();
  }
  function setKind(e, kind) {
    e.kind = kind;
    if (kind === 'track') {
      if (!e.hold) e.hold = 1;
      if (!e.to) e.to = { yaw: e.aim.yaw + 20, pitch: e.aim.pitch, dist: e.aim.dist };
    } else {
      e.hold = 0;
      e.to = null;
    }
    touch();
  }

  const sx = (yaw) => W / 2 - (yaw / YAW) * (W / 2);
  const sy = (pitch) => H / 2 - (pitch / PITCH) * (H / 2);
  const invx = (x) => -((x - W / 2) / (W / 2)) * YAW;
  const invy = (y) => -((y - H / 2) / (H / 2)) * PITCH;
  /** Apparent radius in px: angular size of the weak point at its distance. */
  const px = (e) => Math.max(4, ((Math.atan2(e.radius, e.aim.dist) * 180) / Math.PI / YAW) * (W / 2));

  function pointer(ev) {
    const r = svg.getBoundingClientRect();
    return { x: ((ev.clientX - r.left) / r.width) * W, y: ((ev.clientY - r.top) / r.height) * H };
  }
  function down(ev, k, to) {
    ev.stopPropagation();
    dragging = k;
    dragTo = to;
    dragMoved = false;
    ed.selectedEvent = k;
    svg.setPointerCapture(ev.pointerId);
  }
  function move(ev) {
    if (dragging < 0 || !pattern) return;
    const p = pointer(ev);
    const e = pattern.events[dragging];
    const aim = dragTo ? e.to : e.aim;
    if (!aim) return;
    dragMoved = true;
    aim.yaw = Math.round(Math.max(-YAW, Math.min(YAW, invx(p.x))) * 2) / 2;
    aim.pitch = Math.round(Math.max(-PITCH, Math.min(PITCH, invy(p.y))) * 2) / 2;
    touch();
  }
  function up() {
    dragging = -1;
  }
  function canvasClick(ev) {
    // The click that ends a drag, or one on a weak point, adds nothing.
    if (!pattern || dragging >= 0 || dragMoved || ev.target.tagName === 'circle') {
      dragMoved = false;
      return;
    }
    const p = pointer(ev);
    addEvent(newBeat, newKind, Math.round(invx(p.x)), Math.round(invy(p.y)));
  }
</script>

<div class="panel col">
  <h2>Patterns</h2>
  <div class="row">
    <label>inline</label>
    {#each ed.level.patterns as p, i}
      <button class:sel={ed.selectedPattern === i} onclick={() => select(i)}>{p.name}</button>
    {/each}
    <button onclick={add}>+ new</button>
    <label>copy builtin</label>
    <select onchange={(e) => { duplicate(e.target.value); e.target.value = ''; }}>
      <option value="">…</option>
      {#each ed.builtin.patterns as p}<option value={p.name}>{p.name}</option>{/each}
    </select>
  </div>
  {#if pattern}
    <div class="row">
      <label>name</label><input type="text" bind:value={pattern.name} onchange={touch} />
      <label>length (beats)</label><input type="number" step="1" min="1" bind:value={pattern.length} onchange={touch} />
      <label>add at beat</label><input type="number" step="0.25" min="0" bind:value={newBeat} />
      <select bind:value={newKind}>{#each KINDS as k}<option value={k}>{k}</option>{/each}</select>
      <button onclick={() => addEvent()}>+ event</button>
      <button class="danger" onclick={() => remove(ed.selectedPattern)}>delete pattern</button>
    </div>
    <svg bind:this={svg} viewBox="0 0 {W} {H}" class="aim" onpointermove={move} onpointerup={up} onclick={canvasClick} role="img" aria-label="aim canvas">
      <rect width={W} height={H} fill="#141021" />
      {#each [-60, -40, -20, 0, 20, 40, 60] as y}
        <line x1={sx(y)} y1="0" x2={sx(y)} y2={H} stroke={y === 0 ? '#3d3560' : '#231d38'} />
        <text x={sx(y) + 2} y={H - 4} fill="#8f88a8" font-size="9">{y}°</text>
      {/each}
      {#each [-30, -15, 0, 15, 30] as p}
        <line x1="0" y1={sy(p)} x2={W} y2={sy(p)} stroke={p === 0 ? '#3d3560' : '#231d38'} />
        <text x="2" y={sy(p) - 2} fill="#8f88a8" font-size="9">{p}°</text>
      {/each}
      {#each pattern.events as e, k}
        {#if e.kind === 'track' && e.to}
          <line x1={sx(e.aim.yaw)} y1={sy(e.aim.pitch)} x2={sx(e.to.yaw)} y2={sy(e.to.pitch)} stroke={KIND_COLOR.track} stroke-dasharray="4 3" />
          <circle cx={sx(e.to.yaw)} cy={sy(e.to.pitch)} r={px({ radius: e.radius, aim: e.to })} fill="none" stroke={KIND_COLOR.track} onpointerdown={(ev) => down(ev, k, true)} style="cursor:grab" />
        {/if}
        <circle cx={sx(e.aim.yaw)} cy={sy(e.aim.pitch)} r={px(e)} fill={KIND_COLOR[e.kind]} fill-opacity={ed.selectedEvent === k ? 0.9 : 0.55} stroke={ed.selectedEvent === k ? '#f3e600' : KIND_COLOR[e.kind]} stroke-width={ed.selectedEvent === k ? 2 : 1} onpointerdown={(ev) => down(ev, k, false)} style="cursor:grab" />
        <text x={sx(e.aim.yaw) + px(e) + 3} y={sy(e.aim.pitch) + 4} fill="#e8e3f0" font-size="10">{e.beat}</text>
      {/each}
    </svg>
    <table>
      <thead><tr><th>beat</th><th>kind</th><th>yaw°</th><th>pitch°</th><th>dist m</th><th>radius m</th><th>lead</th><th>hold</th><th>to yaw</th><th>to pitch</th><th></th></tr></thead>
      <tbody>
        {#each pattern.events as e, k}
          <tr class:sel={ed.selectedEvent === k} onclick={() => (ed.selectedEvent = k)}>
            <td><input type="number" step="0.25" bind:value={e.beat} onchange={touch} /></td>
            <td><select value={e.kind} onchange={(ev) => setKind(e, ev.target.value)}>{#each KINDS as kk}<option value={kk}>{kk}</option>{/each}</select></td>
            <td><input type="number" step="0.5" bind:value={e.aim.yaw} onchange={touch} /></td>
            <td><input type="number" step="0.5" bind:value={e.aim.pitch} onchange={touch} /></td>
            <td><input type="number" step="0.5" bind:value={e.aim.dist} onchange={touch} /></td>
            <td><input type="number" step="0.05" bind:value={e.radius} onchange={touch} /></td>
            <td><input type="number" step="0.25" bind:value={e.lead} onchange={touch} /></td>
            <td>{#if e.kind === 'track'}<input type="number" step="0.25" bind:value={e.hold} onchange={touch} />{:else}-{/if}</td>
            <td>{#if e.kind === 'track' && e.to}<input type="number" step="0.5" bind:value={e.to.yaw} onchange={touch} />{:else}-{/if}</td>
            <td>{#if e.kind === 'track' && e.to}<input type="number" step="0.5" bind:value={e.to.pitch} onchange={touch} />{:else}-{/if}</td>
            <td><button class="danger" onclick={() => removeEvent(k)}>×</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <div class="stat">Pick an inline pattern, make a new one, or copy a builtin to edit it. Builtins: {ed.builtin.patterns.map((p) => p.name).join(', ')}.</div>
  {/if}
</div>

<style>
  .aim { width: 100%; max-width: 760px; border: 1px solid var(--line); border-radius: 3px; touch-action: none; }
  tr.sel td { background: #2a2340; }
</style>
