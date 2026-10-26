//! File header, table of contents and chunk tree (spec section 3).

use crate::reader::{Reader, Result, bail};

pub const MAGIC: &str = "BINKIES-MDM";

/// Types whose TOC size covers the following entries instead of owning payload.
const CONTAINER_TYPES: [u32; 2] = [0x02, 0x08];

#[derive(Debug)]
pub enum Chunk<'a> {
    Leaf { ty: u32, data: &'a [u8] },
    Container { ty: u32, children: Vec<Chunk<'a>> },
}

impl Chunk<'_> {
    pub fn ty(&self) -> u32 {
        match self {
            Chunk::Leaf { ty, .. } | Chunk::Container { ty, .. } => *ty,
        }
    }
}

pub struct File<'a> {
    pub version: f32,
    pub chunks: Vec<Chunk<'a>>,
}

pub fn parse(data: &[u8]) -> Result<File<'_>> {
    let mut r = Reader::new(data);
    let magic = r.str()?;
    if magic != MAGIC {
        bail!("bad magic {magic:?}, expected {MAGIC:?}");
    }
    let version = r.f32()?;
    let toc = r.list(|r| Ok((r.u32()?, r.u32()?)))?;

    let mut payload = Reader::new(&data[r.pos()..]);
    let mut entries = toc.into_iter().enumerate().peekable();
    let mut chunks = Vec::new();
    while let Some((i, (ty, size))) = entries.next() {
        if CONTAINER_TYPES.contains(&ty) {
            let mut children = Vec::new();
            let mut covered = 0u64;
            while covered < size as u64 {
                let Some((j, (cty, csize))) = entries.next() else {
                    bail!("container #{i} (type {ty:#x}) runs past the end of the TOC");
                };
                if CONTAINER_TYPES.contains(&cty) {
                    bail!("nested container #{j} (type {cty:#x}) inside #{i}");
                }
                children.push(Chunk::Leaf { ty: cty, data: payload.bytes(csize as usize)? });
                covered += csize as u64;
            }
            if covered != size as u64 {
                bail!("container #{i} (type {ty:#x}) size {size} does not match its children ({covered})");
            }
            chunks.push(Chunk::Container { ty, children });
        } else {
            chunks.push(Chunk::Leaf { ty, data: payload.bytes(size as usize)? });
        }
    }
    payload.finish()?;
    Ok(File { version, chunks })
}
