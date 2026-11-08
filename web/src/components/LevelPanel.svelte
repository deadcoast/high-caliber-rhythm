<script>
  import { ed, touch, allPatterns, resetLevel } from '../lib/editor.svelte.js';
  import { download, normalise, CUE_KINDS } from '../lib/level.js';

  function addTraversal() {
    ed.level.segments.push({ type: 'traversal', bars: 1 });
    touch();
  }
  function addEncounter() {
    const room = ed.level.rooms[0]?.name ?? 'range';
    ed.level.segments.push({ type: 'encounter', room, sections: [{ pattern: 'flick_lr', repeats: 1 }] });
    touch();
  }
  function addBoss() {
    const room = ed.level.rooms[ed.level.rooms.length - 1]?.name ?? 'range';
    ed.level.segments.push({
      type: 'boss',
      room,
      phases: [
        { name: 'phase 1', clear: 0.7, sections: [{ pattern: 'switch_pair', repeats: 4 }] },
        { name: 'phase 2', clear: 0.7, sections: [{ pattern: 'track_arc', repeats: 4 }] },
        { name: 'the core', clear: 0.8, sections: [{ pattern: 'rush_fill', repeats: 2 }] },
      ],
    });
    touch();
  }
  function addPhase(seg) {
    seg.phases.push({ name: 'phase ' + (seg.phases.length + 1), clear: 0.7, sections: [{ pattern: 'flick_quad', repeats: 2 }] });
    touch();
  }
  function removePhase(seg, k) {
    seg.phases.splice(k, 1);
    touch();
  }
  function addCue() {
    if (!ed.level.cues) ed.level.cues = [];
    ed.level.cues.push({ bar: 1, kind: 'lights', room: '', bars: 0, param: 1 });
    touch();
  }
  function removeCue(i) {
    ed.level.cues.splice(i, 1);
    touch();
  }
  function addSectionTo(list) {
    list.push({ pattern: 'flick_lr', repeats: 1 });
    touch();
  }
  function move(i, d) {
    const j = i + d;
    if (j < 0 || j >= ed.level.segments.length) return;
    const s = ed.level.segments;
    [s[i], s[j]] = [s[j], s[i]];
    ed.selectedSegment = j;
    touch();
  }
  function remove(i) {
    ed.level.segments.splice(i, 1);
    ed.selectedSegment = -1;
    touch();
  }
  function addSection(seg) {
    seg.sections.push({ pattern: 'flick_lr', repeats: 1 });
    touch();
  }
  function removeSection(seg, k) {
    seg.sections.splice(k, 1);
    touch();
  }
  function addRoom(name) {
    const b = ed.builtin.rooms.find((r) => r.name === name);
    const room = b ? JSON.parse(JSON.stringify(b)) : { name: 'room' + (ed.level.rooms.length + 1), size: [24, 8, 30], stand: [0, 0, 12], yaw_deg: 0, blocks: [] };
    if (ed.level.rooms.some((r) => r.name === room.name)) room.name += '_' + (ed.level.rooms.length + 1);
    delete room.origin;
    ed.level.rooms.push(room);
    touch();
  }
  function removeRoom(i) {
    ed.level.rooms.splice(i, 1);
    touch();
  }
  function exportLevel() {
    const l = normalise(ed.level);
    download((l.name || 'level') + '.json', JSON.stringify(l, null, 2));
  }
  async function importLevel(e) {
    const f = e.target.files?.[0];
    if (!f) return;
    try {
      const l = JSON.parse(await f.text());
      const fresh = normalise({ ...ed.level, ...l });
      Object.assign(ed.level, fresh);
      for (const k of Object.keys(ed.level)) if (!(k in fresh)) delete ed.level[k];
      touch();
    } catch (err) {
      ed.error = 'import: ' + err;
    }
  }
  // Encounters can only use rooms the level lists; with none listed the
  // compiler supplies the Range room.
  const roomNames = $derived(ed.level.rooms.length ? ed.level.rooms.map((r) => r.name) : ['range']);
</script>

<div class="panel col">
  <h2>Level</h2>
  <div class="row">
    <label>name</label><input type="text" bind:value={ed.level.name} />
    <label>difficulty</label><input type="text" bind:value={ed.level.difficulty} style="width:6em" />
  </div>
  <div class="row">
    <label>song</label><input type="text" bind:value={ed.level.song} />
    <label>artist</label><input type="text" bind:value={ed.level.artist} style="width:8em" />
  </div>
  <div class="row">
    <label>charter</label><input type="text" bind:value={ed.level.charter} style="width:8em" />
    <label>bpm</label><input type="number" step="0.01" bind:value={ed.level.bpm} />
    <label>lead-in beats</label><input type="number" step="1" bind:value={ed.level.lead_in} />
  </div>
  <div class="row">
    <label>audio</label><input type="text" bind:value={ed.level.audio} style="width:14em" placeholder="res://audio/song.ogg" />
    <label>beat 0 s</label><input type="number" step="0.001" bind:value={ed.level.song_offset_s} />
  </div>

  <h3>Rooms <span class="stat">(none listed = the Range room only; add the builtin rooms to use them in the path)</span></h3>
  {#each ed.level.rooms as room, i}
    <div class="row">
      <input type="text" bind:value={room.name} style="width:6em" />
      <label>size</label>
      <input type="number" bind:value={room.size[0]} /><input type="number" bind:value={room.size[1]} /><input type="number" bind:value={room.size[2]} />
      <label>stand</label>
      <input type="number" bind:value={room.stand[0]} /><input type="number" bind:value={room.stand[1]} /><input type="number" bind:value={room.stand[2]} />
      <label>yaw°</label><input type="number" bind:value={room.yaw_deg} />
      <button class="danger" onclick={() => removeRoom(i)}>×</button>
    </div>
  {/each}
  <div class="row">
    <label>add</label>
    {#each ed.builtin.rooms as r}
      <button onclick={() => addRoom(r.name)}>{r.name}</button>
    {/each}
    <button onclick={() => addRoom('')}>custom</button>
  </div>

  <h3>Path <span class="stat">(traversals and encounters, in order)</span></h3>
  {#each ed.level.segments as seg, i}
    <div class="seg" class:sel={ed.selectedSegment === i} role="button" tabindex="0" onclick={() => (ed.selectedSegment = i)} onkeydown={(e) => e.key === 'Enter' && (ed.selectedSegment = i)}>
      <div class="row">
        <span class="stat">#{i + 1}</span>
        {#if seg.type === 'traversal'}
          <b>MOVE</b> <input type="number" step="0.5" bind:value={seg.bars} /> <span class="stat">bars</span>
        {:else if seg.type === 'boss'}
          <b class="kind-rush">BOSS in</b>
          <select bind:value={seg.room}>
            {#each roomNames as n}<option value={n}>{n}</option>{/each}
          </select>
          <span class="stat">a chart within the chart: each phase restarts on its own; a phase below its clear ratio is a down</span>
        {:else}
          <b>FIGHT in</b>
          <select bind:value={seg.room}>
            {#each roomNames as n}<option value={n}>{n}</option>{/each}
          </select>
        {/if}
        <span class="spacer"></span>
        <button onclick={() => move(i, -1)}>↑</button>
        <button onclick={() => move(i, 1)}>↓</button>
        <button class="danger" onclick={() => remove(i)}>×</button>
      </div>
      {#if seg.type === 'encounter'}
        {#each seg.sections as sec, k}
          <div class="row sec">
            <select bind:value={sec.pattern}>
              {#each allPatterns() as p}<option value={p.name}>{p.name}</option>{/each}
            </select>
            <label>×</label><input type="number" min="1" step="1" bind:value={sec.repeats} />
            <button class="danger" onclick={() => removeSection(seg, k)}>×</button>
          </div>
        {/each}
        <div class="row sec"><button onclick={() => addSection(seg)}>+ pattern</button></div>
      {:else if seg.type === 'boss'}
        {#each seg.phases as ph, k}
          <div class="row sec">
            <b class="kind-rush">phase {k + 1}</b>
            <input type="text" bind:value={ph.name} style="width:8em" />
            <label>clear</label><input type="number" min="0" max="1" step="0.05" bind:value={ph.clear} />
            <button class="danger" onclick={() => removePhase(seg, k)}>×</button>
          </div>
          {#each ph.sections as sec, j}
            <div class="row sec2">
              <select bind:value={sec.pattern}>
                {#each allPatterns() as p}<option value={p.name}>{p.name}</option>{/each}
              </select>
              <label>×</label><input type="number" min="1" step="1" bind:value={sec.repeats} />
              <button class="danger" onclick={() => { ph.sections.splice(j, 1); touch(); }}>×</button>
            </div>
          {/each}
          <div class="row sec2"><button onclick={() => addSectionTo(ph.sections)}>+ pattern</button></div>
        {/each}
        <div class="row sec"><button onclick={() => addPhase(seg)}>+ phase</button></div>
      {/if}
    </div>
  {/each}
  <div class="row">
    <button onclick={addTraversal}>+ move</button>
    <button onclick={addEncounter}>+ fight</button>
    <button onclick={addBoss}>+ boss</button>
  </div>

  <h3>Set pieces <span class="stat">(cues on bars; the engine plays them, the model times them)</span></h3>
  {#each ed.level.cues ?? [] as c, i}
    <div class="row">
      <label>bar</label><input type="number" min="1" step="1" bind:value={c.bar} />
      <select bind:value={c.kind}>{#each CUE_KINDS as k}<option value={k}>{k}</option>{/each}</select>
      <select bind:value={c.room}><option value="">-</option>{#each roomNames as n}<option value={n}>{n}</option>{/each}</select>
      <label>bars</label><input type="number" min="0" step="1" bind:value={c.bars} />
      <label>param</label><input type="number" step="0.1" bind:value={c.param} />
      <button class="danger" onclick={() => removeCue(i)}>×</button>
    </div>
  {/each}
  <div class="row"><button onclick={addCue}>+ cue</button></div>

  <h3>File</h3>
  <div class="row">
    <button class="primary" onclick={exportLevel}>export level JSON</button>
    <label class="file">import <input type="file" accept=".json" onchange={importLevel} /></label>
    <button class="danger" onclick={resetLevel}>new</button>
  </div>
  <div class="stat">Drop the JSON in godot/levels/ and run with --level=res://levels/{ed.level.name || 'level'}.json</div>
</div>

<style>
  .seg { border: 1px solid var(--line); border-radius: 3px; padding: 4px 6px; margin-bottom: 4px; cursor: pointer; }
  .sec { padding-left: 24px; margin-top: 3px; }
  .sec2 { padding-left: 48px; margin-top: 3px; }
  .spacer { flex: 1; }
  .file input { width: 12em; }
</style>
