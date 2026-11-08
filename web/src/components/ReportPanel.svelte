<script>
  import { ed, simulate } from '../lib/editor.svelte.js';

  let jitter = $state(22);
  let bias = $state(0);
  let hit = $state(0.92);
  let skip = $state(0.05);
  let perfect = $state(false);

  const r = $derived(ed.chart?.rating);
  const kinds = $derived(ed.chart ? Object.entries(ed.chart.targets.reduce((m, t) => ((m[t.kind] = (m[t.kind] || 0) + 1), m), {})) : []);
  const rep = $derived(ed.sim?.report);
</script>

<div class="panel col">
  <h2>Rating</h2>
  {#if r}
    <div class="stat">stars <b>{r.stars.toFixed(2)}</b></div>
    <div class="stat">aim speed <b>{r.aim_speed.toFixed(1)}</b> °/s · precision <b>{r.precision.toFixed(2)}</b></div>
    <div class="stat">density <b>{r.density.toFixed(2)}</b> /s · tracking <b>{r.tracking.toFixed(2)}</b></div>
    <div class="stat">
      {#each kinds as [k, n]}<span class="kind-{k}">{k} {n}</span>&nbsp; {/each}
    </div>
    <div class="stat">stars = peak strain (top fifth) + 2 × density + 0.5 × tracking, anchored to the warmup at 128 = 3.0</div>
  {:else}
    <div class="stat">{ed.error || 'no chart'}</div>
  {/if}

  <h2 style="margin-top:14px">Bot run</h2>
  <div class="row">
    <label>jitter ms</label><input type="number" bind:value={jitter} />
    <label>bias ms</label><input type="number" bind:value={bias} />
  </div>
  <div class="row">
    <label>hit</label><input type="number" step="0.01" min="0" max="1" bind:value={hit} />
    <label>skip</label><input type="number" step="0.01" min="0" max="1" bind:value={skip} />
    <label><input type="checkbox" bind:checked={perfect} /> perfect</label>
  </div>
  <div class="row">
    <button class="primary" disabled={!ed.chart || ed.simBusy} onclick={() => simulate({ jitter_ms: jitter, bias_ms: bias, hit_prob: hit, skip_prob: skip, perfect })}>{ed.simBusy ? 'running…' : 'simulate'}</button>
    <span class="stat">the same bot as dbsim; a level a jittery bot cannot clear needs a fix before a player sees it</span>
  </div>
  {#if rep}
    <div class="stat">grade <b>{rep.grade}</b> · score <b>{rep.score}</b> · <b>{rep.pp.toFixed(0)}</b> pp {#if rep.pfc}· PFC{:else if rep.fc}· FC{/if}</div>
    <div class="stat">perfect {rep.perfect} · great {rep.great} · good {rep.good} · early {rep.early} · late {rep.late} · miss {rep.expired} · whiff {rep.whiffs}</div>
    <div class="stat">aim <b>{(rep.aim_accuracy * 100).toFixed(1)}%</b> · on-beat <b>{(rep.beat_accuracy * 100).toFixed(1)}%</b> · sd <b>{rep.timing_std_ms.toFixed(1)} ms</b></div>
    <div class="stat">damage <b>{rep.damage_taken}</b> · downs <b>{rep.downs}</b> · restarts <b>{ed.sim.restarts}</b> · max combo <b>{rep.max_combo}</b></div>
    <table>
      <thead><tr><th>kind</th><th>tgts</th><th>hits</th><th>on-beat</th><th>expired</th><th>track</th></tr></thead>
      <tbody>
        {#each Object.entries(rep.by_kind) as [k, s]}
          <tr><td class="kind-{k.toLowerCase()}">{k}</td><td>{s.targets}</td><td>{s.hits}</td><td>{s.on_beat}</td><td>{s.expired}</td><td>{s.track_total ? ((100 * s.track_on) / s.track_total).toFixed(0) + '%' : '-'}</td></tr>
        {/each}
      </tbody>
    </table>
  {/if}

  <h2 style="margin-top:14px">Range drills</h2>
  <div class="stat">{#each ed.builtin.drills as d}<div><b>{d.name}</b> <span class="stat">{d.skill}</span> — {d.description}</div>{/each}</div>
</div>
