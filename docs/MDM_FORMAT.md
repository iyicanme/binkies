# BINKIES-MDM 3.3 file format

Reverse-engineered from `res/geometry.mdm.br` and `res/scene.mdm.br` (a Samsung Galaxy
phone model with five color variants). Every byte in both files is accounted for by this
spec. Confidence levels:

- **Confirmed**: holds for every instance in the samples and cross-checks against other data.
- **Likely**: consistent with the samples, but the meaning is inferred.
- **Unknown**: the layout is known, the meaning is not.

## 1. Overview

- Despite the `.br` extension, the files are **not Brotli-compressed**. They are raw
  little-endian binary. The `.br` was likely added by a transport or CDN step, or the
  name is just misleading.
- One model is split into two files:
  - **geometry**: mesh vertex and index buffers.
  - **scene**: materials, texture references, node hierarchy, decals, color variants.
- Both files use the same container: a header, a flat **table of contents (TOC)** listing
  chunk types and sizes, then the concatenated chunk payloads.
- **Texture pixel data is not in either file.** Textures are referenced by file name
  (section 5.3) and must come from somewhere else.

## 2. Primitive types

| Name     | Encoding |
|----------|----------|
| `u8`     | 1 byte unsigned |
| `u16`    | 2 bytes LE unsigned |
| `u32` / `i32` | 4 bytes LE |
| `f32`    | IEEE-754 single, LE |
| `str`    | .NET `BinaryWriter` string: 7-bit varint byte length (LEB128), then UTF-8 bytes, no terminator |
| `vec2` / `vec3` | 2 / 3 consecutive `f32` |
| `xform`  | 12 × `f32`; see section 5.5 |

The `str` length is *likely* a LEB128 varint (the .NET convention). Every string in the
samples is shorter than 128 bytes, so only the single-byte form has been seen.

## 3. File layout

```
offset  type        field
0       str         magic         = "BINKIES-MDM"   (bytes: 0B 42 49 4E 4B 49 45 53 2D 4D 44 4D)
12      f32         version       = 3.3             (bytes: 33 33 53 40)
16      u32         entry_count
20      TocEntry[entry_count]
...     u8[]        payload       (concatenated leaf-chunk payloads)
EOF
```

```
TocEntry {            // 8 bytes
  u32 type
  u32 size            // bytes; always a multiple of 4
}
```

### 3.1 Containers vs. leaves (confirmed)

The TOC is a pre-order flattening of a shallow tree:

- **Container** types (`0x02` Material, `0x08` Mesh) have **no payload of their own**.
  Their `size` is the sum of the sizes of the entries that follow, and those entries are
  the container's children. Containers do not nest in the samples.
- **Leaf** chunks own `size` bytes of payload. Leaf payloads are stored back to back in
  TOC order, starting right after the TOC. The offset of a leaf is the running sum of the
  preceding *leaf* sizes; container sizes are not counted.

A reader cannot tell containers from leaves using the TOC alone. It has to know the set
of container types.

Check: sum of the leaf sizes = file size − (20 + 8 × entry_count), exactly, for both files.

### 3.2 Padding (confirmed)

Each leaf payload is zero-padded at the end to a multiple of 4 bytes. `size` includes
the padding. Parsers should read the logical fields and skip to `size`.

### 3.3 Type ID encoding (likely)

Type `0x00020009` (index buffer) suggests that the high 16 bits carry a parameter, here
the index element size (2 = `u16`). That would make the base type `0x0009`, and a
`0x00040009` would probably mean `u32` indices. Only `0x00020009` appears in the samples.

## 4. Geometry file

Top level: `Mesh` × N (N = 37 in the sample). Mesh *k* in file order is **mesh index
k**. Scene primitives and decals refer to meshes by this index.

### 4.1 `0x08` Mesh (container)

Its children always appear in this order:

| Type         | Name     | Payload |
|--------------|----------|---------|
| `0x19`       | MeshId   | `str id` |
| `0x0A`       | Positions| `u32 n; vec3[n]` |
| `0x0B`       | Normals  | `u32 n; vec3[n]` (unit length) |
| `0x0C`       | UV0      | `u32 n; vec2[n]` |
| `0x0F`       | VertexScalar | `u32 n; f32[n]` |
| `0x00020009` | Indices  | `u32 m; u16[m]` |

- **MeshId**: an opaque string, usually 15–16 lowercase hex digits (a 64-bit hash
  printed without leading zeros, e.g. `76987791035d18fe`). The two decal meshes have
  short decimal IDs (`5194`, `9819`). Nothing in the scene file refers to these IDs;
  meshes are linked by index.
- All vertex arrays in a mesh have the same `n`.
- **Positions**: model units look like meters (the phone is about 0.07 × 0.16 m).
- **UV0**: not limited to [0,1]. Values such as −7.2 are common, so wrap or repeat
  sampling is expected (see the `textureRepeat` material property).
- **VertexScalar**: one `f32` per vertex in [0, 1.0016]. *Likely* baked ambient
  occlusion or a similar per-vertex attenuation term.
- **Indices**: a triangle list (m % 3 == 0, no degenerate triangles, all < n).
  Front faces are **counter-clockwise**: `cross(b−a, c−a)` agrees with the vertex normal
  for 100% of the triangles tested.
- There is no tangent stream. Tangents for normal mapping have to be derived.

## 5. Scene file

Top-level chunks appear in this order:

| Type   | Count in sample | Name |
|--------|----------------:|------|
| `0x0D` | 35 | Primitive |
| `0x02` | 40 | Material (container) |
| `0x01` | 9  | Texture |
| `0x00` | 41 | Node |
| `0x10` | 5  | Variant |
| `0x11` | 1  | VariantSet |
| `0x14` | 2  | Decal |

Indices count from 0 within each chunk type, in file order (material *k* is the *k*-th
`0x02`, and so on).

### 5.1 `0x0D` Primitive (confirmed)

Binds a geometry mesh to a material.

```
u32 material      // material index
u32 mesh          // geometry mesh index
u32 flags         // observed 0 or 1
u32 reserved      // always 0
```

Primitive *k* always has `mesh == k`. Mesh nodes (5.5) use the same number as `ref`,
so `ref` works as either a primitive index or a mesh index.

`flags` = 1 is set only on `Back_Cam_Lens_Outer`, `Back_Cam_Glass`, `Back_Flash_Ref` and
`Front_Cam_Glass`. Those are all glass or reflector parts, so it *likely* means
**transparent / blended**.

### 5.2 `0x02` Material (container)

Children, in order: one `0x07` header, then `0x03`\*, `0x04`\*, `0x05`\*, `0x18`\*.
Every property chunk stores the **value first, then the name**.

| Type   | Name | Payload |
|--------|------|---------|
| `0x07` | MaterialHeader | `str name; str shader; i32 unknown` |
| `0x03` | ColorProp   | `u8 r, g, b, a; str name` |
| `0x04` | TextureProp | `u32 texture; str name` (texture index, section 5.3) |
| `0x05` | FloatProp   | `f32 value; str name` |
| `0x18` | BoolProp    | `u8 value; str name` |

- `shader` values: `IBL2` (38), `IBL` (1), `PhysicalBased` (1).
- `unknown` is always −1 (unknown).
- ColorProp `a`: observed values are 1 and 255. `baseColor` and `emissiveColor` always
  have 1. `diffuse`, `emissive` and `specularity` have 255 or 1. The meaning is unknown,
  so treat it as raw data rather than as alpha.
- Property names come in two families that often appear side by side: a camelCase PBR
  set (`baseColor`, `normalTexture`, `roughness`, `metallic`, `clearCoat`, `iblType`, …)
  and a lowercase legacy set (`diffuse`, `normal`, `gloss`, `bumpiness`,
  `texturerepeat`, …). A renderer should prefer the camelCase names.
- Names observed:
  - **Colors**: `baseColor`, `emissiveColor`, `diffuse`, `emissive`, `specularity`.
  - **Textures**: `normalTexture`, `baseColorTexture`, `emissiveTexture`, `maskTexture`,
    `iridescentTexture`, `ambientOcclusionTexture`, plus legacy `normal`, `diffuse`,
    `emissive`, `specularity`, `opacity`, `iridescent`, `ambientocclusion`.
  - **Floats**: 36 distinct names, including `opacity`, `metallic`, `roughness`,
    `reflectance`, `anisotropy`, `clearCoat`, `clearCoatRoughness`, `textureRepeat`,
    `iblType`, `concentricNormalFrequency`, `ringcount`, and `*Intensity` /
    `*TextureBias` per texture slot.
  - **Bools**: `mask`, `anisotropicmask`.

### 5.3 `0x01` Texture (layout confirmed, some fields likely)

A reference to an external image. **No pixel data is stored.**

```
str  filename     // e.g. "Substance_graph_normal_3.jpg"
u16  width        // confirmed: power-of-two sizes that match the names (64, 1024, …)
u16  height
u16  unknown      // observed 0x0064, 0x0101, 0x015A, 0x0162, 0x0164, 0x0255, 0x0258
f32  lod_bias     // likely; observed -0.5, -1.0, -2.0
```

`unknown` might be two bytes. The low byte (85–100, or 1) looks like a quality value and
the high byte (0–2) like a format or flag. This is a guess.

### 5.4 `0x14` Decal (confirmed)

```
u32 material      // material index
u32 mesh          // geometry mesh index (the decal meshes, 35 and 36)
u32 reserved      // always 0
```

### 5.5 `0x00` Node (confirmed)

```
u8    kind        // 0 = group, 1 = mesh, 5 = decal
i32   ref         // kind 1: primitive/mesh index; kind 5: decal index; kind 0: -1
i32   parent      // node index, -1 = root
xform transform
str   name
u32   prop_count
{ str key; f32 value }[prop_count]
```

- The sample has four root groups, all with direct children only. Children always come
  after their parent.
- `xform`: every node in the sample stores `(0,0,1) (0,1,0) (1,0,0) (0,0,0)`. That is
  most plausibly three basis vectors plus a translation, but with only identity
  transforms the axis order and row/column convention can't be determined.
  **Treat as identity until a sample with a non-trivial transform exists.**
- Properties on mesh nodes: `aabbMin{X,Y,Z}` / `aabbMax{X,Y,Z}` (a mesh-space bounding
  box that matches the referenced mesh exactly), `Anisotropic{Position,Normal}{X,Y,Z}`,
  `TransparencyOrder`, `Thickness`, `ambientocclusionintensity`.
- Properties on decal nodes: `Decal{Position,Normal}{X,Y,Z}`, `DecalWidth`,
  `DecalPositiveDepth`, `AspectRatio`, `rotation` (degrees, e.g. 315), plus the
  anisotropic and transparency keys. A decal node's parent is the node it is projected
  onto.

### 5.6 `0x10` Variant (confirmed)

A named set of per-node material overrides, used here for the phone colors.

```
str  name         // "Phantom Black"
str  id           // 6-char random ID, e.g. "zmbpeo"
i32  unknown      // always -1 (perhaps an index into 0x11; unconfirmed)
u32  count
VariantEntry[count]
```

```
VariantEntry {    // 59 bytes, not aligned
  i32   node          // node index
  u8    unknown_a     // always 0 (perhaps "override transform")
  xform transform     // always the identity pattern
  u8    has_material  // 1 if material is set
  i32   material      // material index, -1 if none
  u8    unknown_c     // 1 only on entries with has_material = 0
}
```

`unknown_c` = 1 shows up only on `Back_Cam_Decal` entries that carry no material. It
*likely* means **hide this node** in that variant.

### 5.7 `0x11` VariantSet

```
str name          // "Colors"
```

This most likely names the group that all `0x10` variants belong to. The link between
the two is not explicit.

## 6. Resolving the model

1. Parse geometry → `meshes[]`.
2. Parse scene → `primitives[]`, `materials[]`, `textures[]`, `nodes[]`, `decals[]`,
   `variants[]`.
3. For each node:
   - kind 1: draw `meshes[primitives[ref].mesh]` with `materials[primitives[ref].material]`.
   - kind 5: `d = decals[ref]`. Project `meshes[d.mesh]` with `materials[d.material]`
     onto the parent node.
4. To apply a variant, replace each listed node's material with the entry's `material`,
   or hide the node when `unknown_c = 1`.
5. Material texture slots → `textures[i].filename`. Load the images from an external
   source.

## 7. Open questions

- What the `xform` axes and order are (needs a non-identity sample).
- Material header `i32` and Variant `i32` (always −1).
- Texture `unknown` u16 and the meaning of `lod_bias`.
- ColorProp 4th byte (1 vs 255).
- Primitive `flags` beyond bit 0, and Variant `unknown_a` / `unknown_c`.
- Whether index type `0x00040009` (u32) exists.
- Where texture images live: same directory, CDN path, a third file?
- Node names say "Galaxy S21 Ultra" while texture names say "Galaxy S20 FE". The asset
  was probably reused, so don't rely on names.
