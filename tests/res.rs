use binkies::scene::NodeKind;
use binkies::{geometry, obj, scene};

fn load(name: &str) -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/res/").to_string() + name).unwrap()
}

#[test]
fn parses_geometry() {
    let meshes = geometry::parse(&load("geometry.mdm.br")).unwrap();
    assert_eq!(meshes.len(), 37);
    assert_eq!(meshes[0].id, "76987791035d18fe");
    assert_eq!(meshes[0].positions.len(), 49);
    assert_eq!(meshes[0].indices.len(), 240);
    for m in &meshes {
        for n in &m.normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-3, "{}: normal length {len}", m.id);
        }
    }
}

#[test]
fn parses_scene() {
    let s = scene::parse(&load("scene.mdm.br")).unwrap();
    assert_eq!(s.primitives.len(), 35);
    assert_eq!(s.materials.len(), 40);
    assert_eq!(s.textures.len(), 9);
    assert_eq!(s.nodes.len(), 41);
    assert_eq!(s.decals.len(), 2);
    assert_eq!(s.variant_sets, ["Colors"]);
    let names: Vec<_> = s.variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["Phantom Black", "Phantom Silver", "Phantom Titanium", "Phantom Navy", "Phantom Brown"]);
    assert_eq!(s.materials[0].name, "BASE_Black_Pure");
    assert_eq!(s.textures[1].filename, "Substance_graph_normal_3.jpg");
    assert_eq!((s.textures[1].width, s.textures[1].height), (1024, 1024));
    assert_eq!(s.nodes.iter().filter(|n| n.kind == NodeKind::Decal).count(), 2);
}

#[test]
fn node_aabb_matches_mesh_bounds() {
    let meshes = geometry::parse(&load("geometry.mdm.br")).unwrap();
    let s = scene::parse(&load("scene.mdm.br")).unwrap();
    for node in s.nodes.iter().filter(|n| n.kind == NodeKind::Mesh) {
        let mesh = &meshes[s.primitives[node.reference as usize].mesh as usize];
        let prop = |k: &str| node.props.iter().find(|(n, _)| n == k).unwrap().1;
        for (axis, c) in ["X", "Y", "Z"].iter().enumerate() {
            let min = mesh.positions.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min);
            let max = mesh.positions.iter().map(|p| p[axis]).fold(f32::NEG_INFINITY, f32::max);
            assert!((prop(&format!("aabbMin{c}")) - min).abs() < 1e-6, "{}", node.name);
            assert!((prop(&format!("aabbMax{c}")) - max).abs() < 1e-6, "{}", node.name);
        }
    }
}

#[test]
fn truncated_file_is_rejected() {
    let data = load("scene.mdm.br");
    assert!(scene::parse(&data[..data.len() - 4]).is_err());
}

#[test]
fn exports_obj() {
    let meshes = geometry::parse(&load("geometry.mdm.br")).unwrap();
    let s = scene::parse(&load("scene.mdm.br")).unwrap();
    let black = s.variants.iter().position(|v| v.name == "Phantom Black");
    let opts = obj::Options { scale: 1000.0, variant: black, ..Default::default() };
    let (mut o, mut m) = (Vec::new(), Vec::new());
    let stats = obj::export(&meshes, &s, &opts, &mut o, &mut m, "out.mtl").unwrap();
    let (o, m) = (String::from_utf8(o).unwrap(), String::from_utf8(m).unwrap());

    assert_eq!(stats.objects, 35);
    assert!(stats.warnings.is_empty());
    assert_eq!(o.lines().filter(|l| l.starts_with("v ")).count(), stats.vertices);
    assert_eq!(o.lines().filter(|l| l.starts_with("f ")).count(), stats.triangles);

    // Every face index is within the vertices written so far.
    let mut seen = 0;
    for line in o.lines() {
        if line.starts_with("v ") {
            seen += 1;
        } else if let Some(f) = line.strip_prefix("f ") {
            for idx in f.split([' ', '/']) {
                let i: usize = idx.parse().unwrap();
                assert!(i >= 1 && i <= seen, "{line}");
            }
        }
    }

    // The variant replaced the back panel's silver material.
    let back = o.split("\no Back_Panel\n").nth(1).unwrap();
    assert!(back.lines().any(|l| l == "usemtl COLOR_Black_Back"));
    assert!(m.contains("newmtl COLOR_Black_Back"));
    assert!(!m.contains("newmtl COLOR_Silver_Back"));
    // Material names with spaces are sanitized.
    assert!(!o.lines().any(|l| l.starts_with("usemtl ") && l.split(' ').count() != 2));
}
