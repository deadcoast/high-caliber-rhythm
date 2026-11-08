// The wasm core: a JSON-over-C-ABI client for core/src/wasm.rs.
// Works in the browser (fetch) and in Node (bytes), no bindgen.

const enc = new TextEncoder();
const dec = new TextDecoder();

export async function loadCore(source) {
  let module;
  if (source instanceof Uint8Array || source instanceof ArrayBuffer) {
    module = await WebAssembly.compile(source);
  } else {
    const url = source ?? new URL('../../public/downbeat_core.wasm', import.meta.url);
    const res = await fetch(url);
    module = await WebAssembly.compile(await res.arrayBuffer());
  }
  const instance = await WebAssembly.instantiate(module, {});
  const ex = instance.exports;

  function reply(ptr) {
    const len = ex.db_reply_len();
    return JSON.parse(dec.decode(new Uint8Array(ex.memory.buffer, ptr, len)));
  }

  function call(request) {
    const bytes = enc.encode(JSON.stringify(request));
    const p = ex.db_alloc(bytes.length);
    new Uint8Array(ex.memory.buffer, p, bytes.length).set(bytes);
    const out = ex.db_call(p, bytes.length);
    const r = reply(out);
    ex.db_free(p, bytes.length);
    return r;
  }

  /** Tempo, beat 0, energy per bar and an onset track for mono f32 samples. */
  function analyze(samples, sampleRate) {
    const p = ex.db_alloc(samples.length * 4);
    new Float32Array(ex.memory.buffer, p, samples.length).set(samples);
    const out = ex.db_analyze(p, samples.length, sampleRate);
    const r = reply(out);
    ex.db_free(p, samples.length * 4);
    return r;
  }

  return { call, analyze, exports: ex };
}
