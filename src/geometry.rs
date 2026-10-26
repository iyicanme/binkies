//! Geometry file: meshes (spec section 4).

use crate::container::{self, Chunk};
use crate::reader::{Reader, Result, bail};

const MESH: u32 = 0x08;
const MESH_ID: u32 = 0x19;
const POSITIONS: u32 = 0x0a;
const NORMALS: u32 = 0x0b;
const UV0: u32 = 0x0c;
const VERTEX_SCALAR: u32 = 0x0f;
/// Base type of the index buffer; the high 16 bits give the element size.
const INDICES: u32 = 0x09;

#[derive(Debug, Default)]
pub struct Mesh {
    pub id: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Per-vertex value in [0, 1], likely baked ambient occlusion.
    pub vertex_scalar: Vec<f32>,
    /// Triangle list, counter-clockwise front faces.
    pub indices: Vec<u32>,
}

pub fn parse(data: &[u8]) -> Result<Vec<Mesh>> {
    let file = container::parse(data)?;
    file.chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| match chunk {
            Chunk::Container { ty: MESH, children } => {
                parse_mesh(children).map_err(|e| crate::reader::Error(format!("mesh {i}: {e}")))
            }
            other => bail!("unexpected top-level chunk type {:#x} in geometry", other.ty()),
        })
        .collect()
}

fn parse_mesh(children: &[Chunk]) -> Result<Mesh> {
    let mut mesh = Mesh::default();
    for child in children {
        let Chunk::Leaf { ty, data } = child else { unreachable!("containers do not nest") };
        let mut r = Reader::new(data);
        match (*ty & 0xffff, *ty >> 16) {
            (MESH_ID, 0) => mesh.id = r.str()?,
            (POSITIONS, 0) => mesh.positions = r.list(Reader::vec3)?,
            (NORMALS, 0) => mesh.normals = r.list(Reader::vec3)?,
            (UV0, 0) => mesh.uvs = r.list(Reader::vec2)?,
            (VERTEX_SCALAR, 0) => mesh.vertex_scalar = r.list(Reader::f32)?,
            (INDICES, 2) => mesh.indices = r.list(|r| Ok(r.u16()? as u32))?,
            (INDICES, 4) => mesh.indices = r.list(Reader::u32)?,
            _ => bail!("unknown mesh chunk type {ty:#x}"),
        }
        r.finish()?;
    }
    validate(&mesh)?;
    Ok(mesh)
}

fn validate(mesh: &Mesh) -> Result<()> {
    let n = mesh.positions.len();
    for (name, len) in [("normals", mesh.normals.len()), ("uvs", mesh.uvs.len()), ("vertex scalars", mesh.vertex_scalar.len())] {
        if len != n {
            bail!("{} has {len} {name} but {n} positions", mesh.id);
        }
    }
    if !mesh.indices.len().is_multiple_of(3) {
        bail!("{} index count {} is not a multiple of 3", mesh.id, mesh.indices.len());
    }
    if let Some(&bad) = mesh.indices.iter().find(|&&i| i as usize >= n) {
        bail!("{} index {bad} out of range ({n} vertices)", mesh.id);
    }
    Ok(())
}
