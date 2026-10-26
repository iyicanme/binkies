# binkies

Parser and OBJ exporter for **BINKIES-MDM** 3D models: a binary model format made of a
geometry file and a scene file (`*.mdm.br`). The main use is getting these models into
Fusion 360 and other tools that read Wavefront OBJ.

The format was reverse-engineered from the sample files in `res/`. The spec is in
[`docs/MDM_FORMAT.md`](docs/MDM_FORMAT.md).

## Building

Requires Rust 1.87+ (edition 2024). No external dependencies.

```sh
cargo build --release
```

## Usage

```sh
# Export to OBJ + MTL (writes phone.obj and phone.mtl)
cargo run --release -- res/geometry.mdm.br res/scene.mdm.br out/phone.obj

# Apply a color variant
cargo run --release -- res/geometry.mdm.br res/scene.mdm.br out/black.obj --variant "Phantom Black"

# List variants, materials and nodes
cargo run --release -- res/geometry.mdm.br res/scene.mdm.br --list
```

| Option | Description |
|---|---|
| `--units <m\|cm\|mm>` | Output units. Default is `mm`. The source data is in meters. |
| `--variant <name>` | Apply a color variant's material overrides and hidden nodes. Use `--list` to see the names. |
| `--decals` | Include decal meshes (logos and similar). They are excluded by default. |
| `--texture-maps` | Write `map_Kd` / `map_Bump` texture references to the `.mtl`. |
| `--list` | Print the scene contents instead of exporting. |

### Output

- One OBJ object (`o` + `g`) per mesh node, named after the node. Duplicate names get a
  numeric suffix.
- Vertex positions, normals and UVs; triangle faces with counter-clockwise winding.
- One `.mtl` material per scene material that is used. It gets a diffuse color from
  `baseColor` (or `diffuse`), an emissive color, and opacity for transparent parts.

### Importing into Fusion 360

1. Open a design and choose **Insert → Insert Mesh**, then pick the `.obj`.
2. Set the units to match `--units` (millimetres by default).
3. Each part comes in as its own mesh body. To get a solid from a part, use
   **Mesh → Convert Mesh**. This is slow for dense parts, so convert only what you need.
4. The model lies in the XY plane, with its thickness along Z. Rotate it after inserting
   if needed.

## Web app

`web/` holds a static page that runs the same parser and exporter in the browser, compiled
to WebAssembly. You pick the geometry and scene files (both at once, or one at a time;
each file's type is detected automatically), choose the same options as the CLI, and get
a `.zip` with the `.obj` and `.mtl`. Files never leave the browser.

```sh
scripts/build-web.sh              # builds dist/: index.html, app.js, style.css, binkies.wasm
python3 -m http.server -d dist    # then open http://localhost:8000
```

The build needs the WebAssembly target: `rustup target add wasm32-unknown-unknown`.
`src/wasm.rs` holds the WebAssembly interface: plain C-ABI exports, with no wasm-bindgen.

### Deploying to GitHub Pages

`.github/workflows/pages.yml` runs the tests, builds `dist/`, and deploys it on every push
to `main`. You can also run it by hand from the Actions tab. One-time setup: in the
repository's **Settings → Pages**, set **Source** to **GitHub Actions**.

## Limitations

- **The MDM files contain no texture images.** They only reference file names. With
  `--texture-maps`, the `.mtl` points to those names; you have to supply the images.
- **Node transforms are not applied.** Every transform in the sample files is identity,
  and the axis convention is still unknown. If a non-identity transform shows up, the
  exporter prints a warning.
- **Parsing is strict.** Unknown chunk types, leftover bytes, and out-of-range references
  are errors. This is deliberate: a file that doesn't match the spec fails loudly instead
  of producing a broken export.
- Despite the `.br` extension, the files are not Brotli-compressed.

## Library

The crate also works as a library:

```rust
let meshes = binkies::geometry::parse(&std::fs::read("res/geometry.mdm.br")?)?;
let scene = binkies::scene::parse(&std::fs::read("res/scene.mdm.br")?)?;

for node in &scene.nodes {
    println!("{} ({:?})", node.name, node.kind);
}
```

| Module | Contents |
|---|---|
| `container` | File header, table of contents, chunk tree |
| `geometry` | `Mesh`: positions, normals, UVs, per-vertex scalar, indices |
| `scene` | `Scene`: primitives, materials, textures, nodes, decals, variants |
| `obj` | `export()` to OBJ + MTL with `Options` and `Stats` |

## Tests

```sh
cargo test
```

The tests in `tests/res.rs` parse the sample files in `res/`. They check entity counts,
check that every mesh node's bounding box matches its mesh, check that truncated input is
rejected, and verify the OBJ output: index ranges, variant material overrides and name
sanitizing.
