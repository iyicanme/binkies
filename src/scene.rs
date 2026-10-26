//! Scene file: materials, textures, nodes, decals, variants (spec section 5).

use crate::container::{self, Chunk};
use crate::reader::{Error, Reader, Result, bail};

const NODE: u32 = 0x00;
const TEXTURE: u32 = 0x01;
const MATERIAL: u32 = 0x02;
const PRIMITIVE: u32 = 0x0d;
const VARIANT: u32 = 0x10;
const VARIANT_SET: u32 = 0x11;
const DECAL: u32 = 0x14;

const MATERIAL_HEADER: u32 = 0x07;
const COLOR_PROP: u32 = 0x03;
const TEXTURE_PROP: u32 = 0x04;
const FLOAT_PROP: u32 = 0x05;
const BOOL_PROP: u32 = 0x18;

/// The only transform observed in the samples; its axis convention is unknown.
pub const IDENTITY_XFORM: [f32; 12] = [0., 0., 1., 0., 1., 0., 1., 0., 0., 0., 0., 0.];

#[derive(Debug, Default)]
pub struct Scene {
    pub primitives: Vec<Primitive>,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub nodes: Vec<Node>,
    pub decals: Vec<Decal>,
    pub variants: Vec<Variant>,
    pub variant_sets: Vec<String>,
}

#[derive(Debug)]
pub struct Primitive {
    pub material: u32,
    pub mesh: u32,
    /// Bit 0 is likely "transparent".
    pub flags: u32,
}

#[derive(Debug, Default)]
pub struct Material {
    pub name: String,
    pub shader: String,
    pub unknown: i32,
    pub colors: Vec<(String, [u8; 4])>,
    pub textures: Vec<(String, u32)>,
    pub floats: Vec<(String, f32)>,
    pub bools: Vec<(String, bool)>,
}

impl Material {
    pub fn color(&self, name: &str) -> Option<[u8; 4]> {
        self.colors.iter().find(|(n, _)| n == name).map(|(_, c)| *c)
    }

    pub fn texture(&self, name: &str) -> Option<u32> {
        self.textures.iter().find(|(n, _)| n == name).map(|(_, t)| *t)
    }

    pub fn float(&self, name: &str) -> Option<f32> {
        self.floats.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
    }
}

#[derive(Debug)]
pub struct Texture {
    pub filename: String,
    pub width: u16,
    pub height: u16,
    pub unknown: u16,
    pub lod_bias: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Group,
    Mesh,
    Decal,
}

#[derive(Debug)]
pub struct Node {
    pub kind: NodeKind,
    /// Primitive index for mesh nodes, decal index for decal nodes, -1 for groups.
    pub reference: i32,
    pub parent: i32,
    pub transform: [f32; 12],
    pub name: String,
    pub props: Vec<(String, f32)>,
}

#[derive(Debug)]
pub struct Decal {
    pub material: u32,
    pub mesh: u32,
}

#[derive(Debug)]
pub struct Variant {
    pub name: String,
    pub id: String,
    pub unknown: i32,
    pub entries: Vec<VariantEntry>,
}

#[derive(Debug)]
pub struct VariantEntry {
    pub node: u32,
    pub material: Option<u32>,
    /// Likely "hide this node in the variant".
    pub hidden: bool,
}

pub fn parse(data: &[u8]) -> Result<Scene> {
    let file = container::parse(data)?;
    let mut scene = Scene::default();
    for (i, chunk) in file.chunks.iter().enumerate() {
        let context = |e: Error| Error(format!("scene chunk {i} (type {:#x}): {e}", chunk.ty()));
        match chunk {
            Chunk::Container { ty: MATERIAL, children } => {
                scene.materials.push(parse_material(children).map_err(context)?);
            }
            Chunk::Leaf { ty, data } => {
                let mut r = Reader::new(data);
                parse_leaf(&mut scene, *ty, &mut r).and_then(|()| r.finish()).map_err(context)?;
            }
            Chunk::Container { ty, .. } => bail!("unexpected container type {ty:#x} in scene"),
        }
    }
    validate(&scene)?;
    Ok(scene)
}

fn parse_leaf(scene: &mut Scene, ty: u32, r: &mut Reader) -> Result<()> {
    match ty {
        PRIMITIVE => {
            let p = Primitive { material: r.u32()?, mesh: r.u32()?, flags: r.u32()? };
            r.u32()?; // reserved
            scene.primitives.push(p);
        }
        TEXTURE => scene.textures.push(Texture {
            filename: r.str()?,
            width: r.u16()?,
            height: r.u16()?,
            unknown: r.u16()?,
            lod_bias: r.f32()?,
        }),
        NODE => {
            let kind = match r.u8()? {
                0 => NodeKind::Group,
                1 => NodeKind::Mesh,
                5 => NodeKind::Decal,
                k => bail!("unknown node kind {k}"),
            };
            scene.nodes.push(Node {
                kind,
                reference: r.i32()?,
                parent: r.i32()?,
                transform: r.xform()?,
                name: r.str()?,
                props: r.list(|r| Ok((r.str()?, r.f32()?)))?,
            });
        }
        DECAL => {
            let d = Decal { material: r.u32()?, mesh: r.u32()? };
            r.u32()?; // reserved
            scene.decals.push(d);
        }
        VARIANT => scene.variants.push(Variant {
            name: r.str()?,
            id: r.str()?,
            unknown: r.i32()?,
            entries: r.list(parse_variant_entry)?,
        }),
        VARIANT_SET => scene.variant_sets.push(r.str()?),
        _ => bail!("unknown chunk type"),
    }
    Ok(())
}

fn parse_variant_entry(r: &mut Reader) -> Result<VariantEntry> {
    let node = r.u32()?;
    let _unknown_a = r.u8()?;
    let _transform = r.xform()?;
    let has_material = r.u8()? != 0;
    let material = r.i32()?;
    let hidden = r.u8()? != 0;
    let material = match (has_material, u32::try_from(material)) {
        (true, Ok(m)) => Some(m),
        (true, Err(_)) => bail!("variant entry for node {node} has a material flag but index {material}"),
        (false, _) => None,
    };
    Ok(VariantEntry { node, material, hidden })
}

fn parse_material(children: &[Chunk]) -> Result<Material> {
    let mut m = Material::default();
    for (i, child) in children.iter().enumerate() {
        let Chunk::Leaf { ty, data } = child else { unreachable!("containers do not nest") };
        let mut r = Reader::new(data);
        match *ty {
            MATERIAL_HEADER if i == 0 => {
                m.name = r.str()?;
                m.shader = r.str()?;
                m.unknown = r.i32()?;
            }
            _ if i == 0 => bail!("material does not start with a header chunk"),
            COLOR_PROP => {
                let rgba = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
                m.colors.push((r.str()?, rgba));
            }
            TEXTURE_PROP => {
                let t = r.u32()?;
                m.textures.push((r.str()?, t));
            }
            FLOAT_PROP => {
                let v = r.f32()?;
                m.floats.push((r.str()?, v));
            }
            BOOL_PROP => {
                let v = r.u8()? != 0;
                m.bools.push((r.str()?, v));
            }
            _ => bail!("unknown material chunk type {ty:#x}"),
        }
        r.finish()?;
    }
    Ok(m)
}

/// Checks cross-references that don't need the geometry file.
fn validate(s: &Scene) -> Result<()> {
    let check = |what: &str, index: i64, len: usize| -> Result<()> {
        if index < 0 || index as usize >= len {
            bail!("{what} index {index} out of range (0..{len})");
        }
        Ok(())
    };
    for p in &s.primitives {
        check("primitive material", p.material.into(), s.materials.len())?;
    }
    for d in &s.decals {
        check("decal material", d.material.into(), s.materials.len())?;
    }
    for m in &s.materials {
        for (_, t) in &m.textures {
            check(&format!("material {:?} texture", m.name), (*t).into(), s.textures.len())?;
        }
    }
    for n in &s.nodes {
        match n.kind {
            NodeKind::Mesh => check(&format!("node {:?} primitive", n.name), n.reference.into(), s.primitives.len())?,
            NodeKind::Decal => check(&format!("node {:?} decal", n.name), n.reference.into(), s.decals.len())?,
            NodeKind::Group => {}
        }
        if n.parent != -1 {
            check(&format!("node {:?} parent", n.name), n.parent.into(), s.nodes.len())?;
        }
    }
    for v in &s.variants {
        for e in &v.entries {
            check(&format!("variant {:?} node", v.name), e.node.into(), s.nodes.len())?;
            if let Some(m) = e.material {
                check(&format!("variant {:?} material", v.name), m.into(), s.materials.len())?;
            }
        }
    }
    Ok(())
}
