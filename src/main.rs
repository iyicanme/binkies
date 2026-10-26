use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use binkies::scene::{NodeKind, Scene};
use binkies::{geometry, obj, scene};

const USAGE: &str = "\
usage: binkies <geometry.mdm> <scene.mdm> <out.obj> [options]
       binkies <geometry.mdm> <scene.mdm> --list

options:
  --units <m|mm|cm>    output units (default: mm; source data is in meters)
  --variant <name>     apply a color variant, e.g. \"Phantom Black\"
  --decals             include decal meshes
  --texture-maps       reference texture files in the .mtl (images are not in the MDM files)
  --list               print variants, materials and nodes instead of exporting";

struct Args {
    geometry: PathBuf,
    scene: PathBuf,
    output: Option<PathBuf>,
    list: bool,
    scale: f32,
    variant: Option<String>,
    decals: bool,
    texture_maps: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut args = Args {
        geometry: PathBuf::new(),
        scene: PathBuf::new(),
        output: None,
        list: false,
        scale: 1000.0,
        variant: None,
        decals: false,
        texture_maps: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--units" => {
                args.scale = match it.next().as_deref() {
                    Some("m") => 1.0,
                    Some("cm") => 100.0,
                    Some("mm") => 1000.0,
                    other => return Err(format!("invalid --units {other:?}")),
                }
            }
            "--variant" => args.variant = Some(it.next().ok_or("--variant needs a name")?),
            "--decals" => args.decals = true,
            "--texture-maps" => args.texture_maps = true,
            "--list" => args.list = true,
            "-h" | "--help" => return Err(String::new()),
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ => positional.push(PathBuf::from(arg)),
        }
    }
    let expected = if args.list { 2 } else { 3 };
    if positional.len() != expected {
        return Err(format!("expected {expected} paths, got {}", positional.len()));
    }
    let mut positional = positional.into_iter();
    args.geometry = positional.next().unwrap();
    args.scene = positional.next().unwrap();
    args.output = positional.next();
    Ok(args)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("error: {e}\n");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn run(args: &Args) -> Result<(), String> {
    let meshes = geometry::parse(&read(&args.geometry)?).map_err(|e| format!("{}: {e}", args.geometry.display()))?;
    let scene = scene::parse(&read(&args.scene)?).map_err(|e| format!("{}: {e}", args.scene.display()))?;

    if args.list {
        list(&scene);
        return Ok(());
    }

    let variant = match &args.variant {
        Some(name) => Some(scene.variants.iter().position(|v| &v.name == name).ok_or_else(|| {
            let names: Vec<_> = scene.variants.iter().map(|v| v.name.as_str()).collect();
            format!("no variant {name:?}; available: {names:?}")
        })?),
        None => None,
    };
    let obj_path = args.output.as_ref().unwrap();
    let mtl_path = obj_path.with_extension("mtl");
    let mtl_name = mtl_path.file_name().unwrap().to_string_lossy().into_owned();
    let create = |p: &Path| File::create(p).map(BufWriter::new).map_err(|e| format!("{}: {e}", p.display()));

    let opts = obj::Options { scale: args.scale, variant, include_decals: args.decals, texture_maps: args.texture_maps };
    let stats = obj::export(&meshes, &scene, &opts, &mut create(obj_path)?, &mut create(&mtl_path)?, &mtl_name)
        .map_err(|e| e.to_string())?;

    for w in &stats.warnings {
        eprintln!("warning: {w}");
    }
    println!(
        "wrote {} and {}: {} objects, {} vertices, {} triangles",
        obj_path.display(),
        mtl_path.display(),
        stats.objects,
        stats.vertices,
        stats.triangles
    );
    Ok(())
}

fn list(scene: &Scene) {
    println!("variants:");
    for v in &scene.variants {
        println!("  {:?} ({} overrides)", v.name, v.entries.len());
    }
    println!("materials:");
    for (i, m) in scene.materials.iter().enumerate() {
        println!("  {i:3} {} [{}]", m.name, m.shader);
    }
    println!("nodes:");
    for (i, n) in scene.nodes.iter().enumerate() {
        let detail = match n.kind {
            NodeKind::Group => "group".to_string(),
            NodeKind::Mesh => {
                let p = &scene.primitives[n.reference as usize];
                format!("mesh {} material {}", p.mesh, scene.materials[p.material as usize].name)
            }
            NodeKind::Decal => {
                let d = &scene.decals[n.reference as usize];
                format!("decal mesh {} material {}", d.mesh, scene.materials[d.material as usize].name)
            }
        };
        println!("  {i:3} {:<36} parent {:3}  {detail}", n.name, n.parent);
    }
}
