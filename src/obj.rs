//! Wavefront OBJ + MTL export.

use std::collections::{BTreeSet, HashSet};
use std::io::Write;

use crate::geometry::Mesh;
use crate::reader::{Result, bail};
use crate::scene::{IDENTITY_XFORM, Material, NodeKind, Scene};

pub struct Options {
    /// Multiplier for positions; the source is in meters.
    pub scale: f32,
    /// Index into `Scene::variants` whose material overrides to apply.
    pub variant: Option<usize>,
    pub include_decals: bool,
    /// Emit `map_Kd` / `map_Bump` references. The images are not in the MDM files.
    pub texture_maps: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { scale: 1.0, variant: None, include_decals: false, texture_maps: false }
    }
}

#[derive(Debug, Default)]
pub struct Stats {
    pub objects: usize,
    pub triangles: usize,
    pub vertices: usize,
    pub warnings: Vec<String>,
}

/// Writes one OBJ object per visible mesh (and optionally decal) node.
pub fn export(
    meshes: &[Mesh],
    scene: &Scene,
    opts: &Options,
    obj: &mut impl Write,
    mtl: &mut impl Write,
    mtl_filename: &str,
) -> Result<Stats> {
    let mut stats = Stats::default();
    let variant = match opts.variant {
        Some(v) => Some(scene.variants.get(v).ok_or_else(|| crate::reader::Error(format!("no variant {v}")))?),
        None => None,
    };
    let material_names = unique_names(scene.materials.iter().map(|m| m.name.as_str()));
    let mut object_names = HashSet::new();
    let mut used_materials = BTreeSet::new();

    writeln!(obj, "# Exported by binkies from BINKIES-MDM")?;
    if let Some(v) = variant {
        writeln!(obj, "# Variant: {}", v.name)?;
    }
    writeln!(obj, "mtllib {mtl_filename}")?;

    // OBJ indices are global and 1-based.
    let mut base = 1usize;
    for (node_index, node) in scene.nodes.iter().enumerate() {
        let (mesh_index, mut material) = match node.kind {
            NodeKind::Group => continue,
            NodeKind::Decal if !opts.include_decals => continue,
            NodeKind::Mesh => {
                let p = &scene.primitives[node.reference as usize];
                (p.mesh, p.material)
            }
            NodeKind::Decal => {
                let d = &scene.decals[node.reference as usize];
                (d.mesh, d.material)
            }
        };
        if let Some(entry) = variant.and_then(|v| v.entries.iter().find(|e| e.node as usize == node_index)) {
            if entry.hidden {
                continue;
            }
            if let Some(m) = entry.material {
                material = m;
            }
        }
        let Some(mesh) = meshes.get(mesh_index as usize) else {
            bail!("node {:?} references mesh {mesh_index}, geometry has {}", node.name, meshes.len());
        };
        if node.transform != IDENTITY_XFORM {
            stats.warnings.push(format!("node {:?} has a non-identity transform; exported untransformed", node.name));
        }

        let name = unique(&sanitize(&node.name), &mut object_names);
        writeln!(obj, "\no {name}\ng {name}")?;
        for p in &mesh.positions {
            writeln!(obj, "v {} {} {}", p[0] * opts.scale, p[1] * opts.scale, p[2] * opts.scale)?;
        }
        for t in &mesh.uvs {
            writeln!(obj, "vt {} {}", t[0], t[1])?;
        }
        for n in &mesh.normals {
            writeln!(obj, "vn {} {} {}", n[0], n[1], n[2])?;
        }
        writeln!(obj, "usemtl {}", material_names[material as usize])?;
        for tri in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| i as usize + base);
            writeln!(obj, "f {a}/{a}/{a} {b}/{b}/{b} {c}/{c}/{c}")?;
        }

        base += mesh.positions.len();
        used_materials.insert(material as usize);
        stats.objects += 1;
        stats.vertices += mesh.positions.len();
        stats.triangles += mesh.indices.len() / 3;
    }

    writeln!(mtl, "# Exported by binkies from BINKIES-MDM")?;
    for &m in &used_materials {
        write_material(mtl, &material_names[m], &scene.materials[m], scene, opts)?;
    }
    Ok(stats)
}

fn write_material(out: &mut impl Write, name: &str, m: &Material, scene: &Scene, opts: &Options) -> Result<()> {
    let rgb = |c: [u8; 4]| format!("{} {} {}", c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0);
    writeln!(out, "\nnewmtl {name}")?;
    let base = m.color("baseColor").or_else(|| m.color("diffuse")).unwrap_or([204, 204, 204, 255]);
    writeln!(out, "Kd {}", rgb(base))?;
    if let Some(e) = m.color("emissiveColor").filter(|e| e[..3] != [0, 0, 0]) {
        writeln!(out, "Ke {}", rgb(e))?;
    }
    let opacity = m.float("opacity").unwrap_or(1.0);
    if opacity < 1.0 {
        writeln!(out, "d {opacity}")?;
    }
    writeln!(out, "illum 2")?;
    if opts.texture_maps {
        let maps = [("map_Kd", ["baseColorTexture", "diffuse"]), ("map_Bump", ["normalTexture", "normal"])];
        for (key, slots) in maps {
            if let Some(t) = slots.iter().find_map(|s| m.texture(s)) {
                writeln!(out, "{key} {}", scene.textures[t as usize].filename)?;
            }
        }
    }
    Ok(())
}

/// OBJ/MTL names end at whitespace.
fn sanitize(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_whitespace() || c.is_control() { '_' } else { c }).collect();
    if s.is_empty() { "unnamed".into() } else { s }
}

fn unique(name: &str, taken: &mut HashSet<String>) -> String {
    let mut candidate = name.to_string();
    let mut n = 2;
    while !taken.insert(candidate.clone()) {
        candidate = format!("{name}_{n}");
        n += 1;
    }
    candidate
}

fn unique_names<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut taken = HashSet::new();
    names.map(|n| unique(&sanitize(n), &mut taken)).collect()
}
