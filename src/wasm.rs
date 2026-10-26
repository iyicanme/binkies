//! Raw C-ABI exports for the web app (`web/`), built for `wasm32-unknown-unknown`.
//!
//! Protocol: JS copies input into memory from [`alloc`] and passes ownership to a call.
//! Each call returns 0 on success or 1 on error. It leaves a UTF-8 message (JSON on
//! success, error text on failure) in the message buffer and any binary result in the
//! output buffer.

use std::cell::RefCell;

use crate::container::{self, Chunk};
use crate::geometry::{self, Mesh};
use crate::obj;
use crate::scene::{self, Scene};
use crate::zip;

/// Top-level chunk type of geometry files (Mesh container).
const MESH_CONTAINER: u32 = 0x08;

#[derive(Default)]
struct State {
    meshes: Option<Vec<Mesh>>,
    scene: Option<Scene>,
    message: String,
    output: Vec<u8>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr` must come from [`alloc`] with `len` bytes, all initialized by the caller.
unsafe fn take(ptr: *mut u8, len: usize) -> Vec<u8> {
    unsafe { Vec::from_raw_parts(ptr, len, len) }
}

/// Parses a geometry or scene file (auto-detected) and keeps it for [`export`].
/// The message is a JSON summary of everything loaded so far.
///
/// # Safety
/// See [`take`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn load(ptr: *mut u8, len: usize) -> u32 {
    let data = unsafe { take(ptr, len) };
    with_state(|s| {
        let file = container::parse(&data).map_err(|e| e.to_string())?;
        let is_geometry =
            !file.chunks.is_empty() && file.chunks.iter().all(|c| matches!(c, Chunk::Container { ty: MESH_CONTAINER, .. }));
        if is_geometry {
            s.meshes = Some(geometry::parse(&data).map_err(|e| format!("geometry: {e}"))?);
        } else {
            s.scene = Some(scene::parse(&data).map_err(|e| format!("scene: {e}"))?);
        }
        s.message = summary(s, if is_geometry { "geometry" } else { "scene" });
        Ok(())
    })
}

/// Exports the loaded model as a ZIP holding `<name>.obj` and `<name>.mtl`.
/// `variant` < 0 means no variant. The message is a JSON summary of the export.
///
/// # Safety
/// See [`take`]; `name` must be UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export(
    scale: f32,
    variant: i32,
    include_decals: u32,
    texture_maps: u32,
    name_ptr: *mut u8,
    name_len: usize,
) -> u32 {
    let name = unsafe { take(name_ptr, name_len) };
    with_state(|s| {
        let name = String::from_utf8(name).map_err(|e| e.to_string())?;
        let (Some(meshes), Some(scene)) = (&s.meshes, &s.scene) else {
            return Err("both the geometry and the scene file are needed".into());
        };
        let opts = obj::Options {
            scale,
            variant: usize::try_from(variant).ok(),
            include_decals: include_decals != 0,
            texture_maps: texture_maps != 0,
        };
        let mtl_name = format!("{name}.mtl");
        let (mut obj_buf, mut mtl_buf) = (Vec::new(), Vec::new());
        let stats = obj::export(meshes, scene, &opts, &mut obj_buf, &mut mtl_buf, &mtl_name).map_err(|e| e.to_string())?;
        s.output = zip::store(&[(&format!("{name}.obj"), &obj_buf), (&mtl_name, &mtl_buf)]);
        let warnings: Vec<_> = stats.warnings.iter().map(|w| json_str(w)).collect();
        s.message = format!(
            r#"{{"objects":{},"vertices":{},"triangles":{},"warnings":[{}]}}"#,
            stats.objects,
            stats.vertices,
            stats.triangles,
            warnings.join(",")
        );
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn message_ptr() -> *const u8 {
    STATE.with_borrow(|s| s.message.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn message_len() -> usize {
    STATE.with_borrow(|s| s.message.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn output_ptr() -> *const u8 {
    STATE.with_borrow(|s| s.output.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn output_len() -> usize {
    STATE.with_borrow(|s| s.output.len())
}

fn with_state(f: impl FnOnce(&mut State) -> Result<(), String>) -> u32 {
    STATE.with_borrow_mut(|s| {
        s.output = Vec::new();
        match f(s) {
            Ok(()) => 0,
            Err(e) => {
                s.message = e;
                1
            }
        }
    })
}

fn summary(s: &State, loaded: &str) -> String {
    let geometry = match &s.meshes {
        Some(m) => {
            let tris: usize = m.iter().map(|m| m.indices.len() / 3).sum();
            format!(r#"{{"meshes":{},"triangles":{tris}}}"#, m.len())
        }
        None => "null".into(),
    };
    let scene = match &s.scene {
        Some(sc) => {
            let variants: Vec<_> = sc.variants.iter().map(|v| json_str(&v.name)).collect();
            format!(
                r#"{{"nodes":{},"materials":{},"decals":{},"variants":[{}]}}"#,
                sc.nodes.len(),
                sc.materials.len(),
                sc.decals.len(),
                variants.join(",")
            )
        }
        None => "null".into(),
    };
    format!(r#"{{"loaded":"{loaded}","geometry":{geometry},"scene":{scene}}}"#)
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
