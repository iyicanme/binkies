// UI for the binkies WebAssembly module. See src/wasm.rs for the calling protocol.

const $ = (id) => document.getElementById(id);
const decoder = new TextDecoder();
const encoder = new TextEncoder();

const wasmModule = await compileWasm();
let wasm = await instantiate();
const loaded = { geometry: null, scene: null };

async function compileWasm() {
  const response = fetch("binkies.wasm");
  try {
    return await WebAssembly.compileStreaming(response);
  } catch {
    // Servers that send the wrong MIME type break streaming compilation.
    return WebAssembly.compile(await (await response).arrayBuffer());
  }
}

async function instantiate() {
  return (await WebAssembly.instantiate(wasmModule, {})).exports;
}

function passBytes(bytes) {
  const ptr = wasm.alloc(bytes.length);
  new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
}

function message() {
  return decoder.decode(new Uint8Array(wasm.memory.buffer, wasm.message_ptr(), wasm.message_len()));
}

/** Runs a wasm call; returns the parsed JSON message or throws with the error text. */
async function call(fn) {
  let status;
  try {
    status = fn();
  } catch (e) {
    // A trap leaves the module in an undefined state: start over with a fresh instance.
    wasm = await instantiate();
    loaded.geometry = loaded.scene = null;
    render();
    throw new Error(`Internal error (${e.message}). Please pick the files again.`);
  }
  if (status !== 0) throw new Error(message());
  return JSON.parse(message());
}

async function loadFiles(files) {
  for (const file of files) {
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const info = await call(() => wasm.load(...passBytes(bytes)));
      loaded[info.loaded] = { file: file.name, info: info[info.loaded] };
      setStatus("");
    } catch (e) {
      setStatus(`${file.name}: ${e.message}`, "error");
    }
  }
  render();
}

function render() {
  const describe = {
    geometry: (i) => `${i.meshes} meshes, ${i.triangles.toLocaleString()} triangles`,
    scene: (i) => `${i.nodes} nodes, ${i.materials} materials, ${i.variants.length} variants`,
  };
  for (const li of $("files").children) {
    const entry = loaded[li.dataset.kind];
    li.classList.toggle("ok", !!entry);
    li.querySelector(".file-state").textContent = entry
      ? `${entry.file} · ${describe[li.dataset.kind](entry.info)}`
      : "Not loaded";
  }

  const variant = $("variant");
  const current = variant.value;
  variant.length = 1;
  (loaded.scene?.info.variants ?? []).forEach((name, i) => variant.add(new Option(name, i)));
  variant.value = [...variant.options].some((o) => o.value === current) ? current : "-1";

  const ready = !!(loaded.geometry && loaded.scene);
  $("options").disabled = !ready;
  $("export").disabled = !ready;
}

async function exportZip() {
  const name = $("name").value.trim().replace(/[\\/:*?"<>|\s]+/g, "_") || "model";
  try {
    const stats = await call(() =>
      wasm.export(
        Number($("units").value),
        Number($("variant").value),
        $("decals").checked ? 1 : 0,
        $("texture-maps").checked ? 1 : 0,
        ...passBytes(encoder.encode(name)),
      ),
    );
    const zip = new Uint8Array(wasm.memory.buffer, wasm.output_ptr(), wasm.output_len()).slice();
    download(new Blob([zip], { type: "application/zip" }), `${name}.zip`);
    const warnings = stats.warnings.length ? ` Warnings: ${stats.warnings.join("; ")}` : "";
    setStatus(
      `Exported ${stats.objects} parts, ${stats.vertices.toLocaleString()} vertices, ${stats.triangles.toLocaleString()} triangles.${warnings}`,
      "ok",
    );
  } catch (e) {
    setStatus(e.message, "error");
  }
}

function download(blob, filename) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
}

function setStatus(text, kind = "") {
  $("status").textContent = text;
  $("status").className = `status ${kind}`;
}

$("file-input").addEventListener("change", (e) => {
  loadFiles([...e.target.files]);
  e.target.value = "";
});

const drop = $("drop");
drop.addEventListener("dragover", (e) => {
  e.preventDefault();
  drop.classList.add("over");
});
drop.addEventListener("dragleave", () => drop.classList.remove("over"));
drop.addEventListener("drop", (e) => {
  e.preventDefault();
  drop.classList.remove("over");
  loadFiles([...e.dataTransfer.files]);
});

$("name").addEventListener("input", () => {
  const name = $("name").value.trim() || "model";
  document.querySelectorAll(".name-echo").forEach((el) => (el.textContent = name));
});

$("export").addEventListener("click", exportZip);
