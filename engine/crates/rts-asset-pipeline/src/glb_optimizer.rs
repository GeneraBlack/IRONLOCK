mod binary_io;
mod hierarchy;
mod inspect;
mod optimize_pass;
mod qem;
mod qem_math;

pub use binary_io::{encode_glb_chunks, parse_glb_chunks};
pub use inspect::{
    extract_world_triangles_from_glb, inspect_glb_bytes, map_clips_to_anim_clip_map,
};
pub use optimize_pass::{optimize_glb_bytes, optimize_mod_models};

use rts_sdk::schema::AnimClipMap;

pub const GLTF_MAGIC: u32 = 0x4654_6C67;
pub const GLTF_VERSION: u32 = 2;
pub const CHUNK_JSON: u32 = 0x4E4F_534A;
pub const CHUNK_BIN: u32 = 0x004E_4942;

const COMP_BYTE: u32 = 5120;
const COMP_UBYTE: u32 = 5121;
const COMP_SHORT: u32 = 5122;
const COMP_USHORT: u32 = 5123;
const COMP_UINT: u32 = 5125;
const COMP_FLOAT: u32 = 5126;

#[derive(Debug, Clone)]
pub struct GlbOptimizeOptions {
    pub max_unit_verts: usize,
    pub max_building_verts: usize,
    pub lod1_ratio: f32,
    pub force: bool,
}

impl Default for GlbOptimizeOptions {
    fn default() -> Self {
        Self {
            max_unit_verts: 6_000,
            max_building_verts: 12_000,
            lod1_ratio: 0.22,
            force: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GlbOptimizationReport {
    pub original_vertices: usize,
    pub lod0_vertices: usize,
    pub lod1_vertices: usize,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub detected_sockets: Vec<(String, [f32; 3])>,
    pub animation_clips: Vec<String>,
    pub skipped_already_optimized: bool,
}

#[derive(Debug, Clone)]
pub struct GlbInspection {
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub lod0_vertices: usize,
    pub lod1_vertices: usize,
    pub has_lod1_for_all_meshes: bool,
    pub has_color_0: bool,
    pub has_turret: bool,
    pub has_barrel: bool,
    pub has_rotor: bool,
    pub detected_sockets: Vec<(String, [f32; 3])>,
    pub animation_clips: Vec<String>,
    pub mapped_animations: AnimClipMap,
}

#[derive(Debug, Clone)]
pub struct WorldRenderTri {
    pub p: [[f32; 3]; 3],
    pub n: [[f32; 3]; 3],
    pub rgba: [[f32; 4]; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub double_sided: bool,
    pub is_primary_slot: bool,
}

#[derive(Debug, Clone)]
struct DecodedPrimitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Option<Vec<[f32; 4]>>,
    pub texcoords: Option<Vec<[f32; 2]>>,
    pub indices: Vec<u32>,
    pub material: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use binary_io::append_encoded_primitive;
    use optimize_pass::collect_glb_files_recursive;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn make_dense_articulated_tank_glb(radial_segs: usize, rings: usize) -> Vec<u8> {
        let build_sphere = |center: [f32; 3], radius: f32, col: [f32; 4]| {
            let mut pos = Vec::new();
            let mut nrm = Vec::new();
            let mut cols = Vec::new();
            let mut idx = Vec::new();
            for r in 0..=rings {
                let phi = std::f32::consts::PI * (r as f32 / rings as f32);
                let (sp, cp) = phi.sin_cos();
                for s in 0..=radial_segs {
                    let theta = std::f32::consts::TAU * (s as f32 / radial_segs as f32);
                    let (st, ct) = theta.sin_cos();
                    let nx = sp * ct;
                    let ny = cp;
                    let nz = sp * st;
                    pos.push([
                        center[0] + radius * nx,
                        center[1] + radius * ny,
                        center[2] + radius * nz,
                    ]);
                    nrm.push([nx, ny, nz]);
                    cols.push(col);
                }
            }
            let stride = (radial_segs + 1) as u32;
            for r in 0..(rings as u32) {
                for s in 0..(radial_segs as u32) {
                    let a = r * stride + s;
                    let b = a + 1;
                    let c = (r + 1) * stride + s;
                    let d = c + 1;
                    idx.extend_from_slice(&[a, b, c, b, d, c]);
                }
            }
            DecodedPrimitive {
                positions: pos,
                normals: nrm,
                colors: Some(cols),
                texcoords: None,
                indices: idx,
                material: Some(0),
            }
        };

        let hull_p = build_sphere([0.0, 0.5, 0.0], 1.2, [0.18, 0.45, 0.88, 1.0]);
        let tur_p = build_sphere([0.0, 1.1, 0.1], 0.65, [0.22, 0.52, 0.92, 1.0]);
        let bar_p = build_sphere([0.0, 1.15, 0.95], 0.22, [0.25, 0.26, 0.28, 1.0]);

        let mut bin = Vec::new();
        let mut bvs = Vec::new();
        let mut accs = Vec::new();
        let m0 = append_encoded_primitive(&hull_p, &mut bin, &mut bvs, &mut accs);
        let m1 = append_encoded_primitive(&tur_p, &mut bin, &mut bvs, &mut accs);
        let m2 = append_encoded_primitive(&bar_p, &mut bin, &mut bvs, &mut accs);

        let root = json!({
            "asset": { "version": "2.0", "generator": "unit-test" },
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": [
                { "name": "Hull", "mesh": 0, "children": [1] },
                { "name": "Turret", "mesh": 1, "children": [2] },
                { "name": "Barrel", "mesh": 2, "children": [3] },
                { "name": "Muzzle_01", "translation": [0.0, 1.15, 1.6] }
            ],
            "materials": [{
                "name": "TestMat",
                "pbrMetallicRoughness": { "baseColorFactor": [1.0, 1.0, 1.0, 1.0] }
            }],
            "meshes": [
                { "name": "hull_mesh", "primitives": [m0] },
                { "name": "turret_mesh", "primitives": [m1] },
                { "name": "barrel_mesh", "primitives": [m2] }
            ],
            "bufferViews": bvs,
            "accessors": accs
        });
        encode_glb_chunks(root, bin).expect("valid test glb")
    }

    #[test]
    fn qem_optimizer_reduces_dense_articulated_glb_embeds_lod1_and_is_idempotent() {
        let raw = make_dense_articulated_tank_glb(96, 64);
        let pre = inspect_glb_bytes(&raw).expect("inspect raw");
        assert!(pre.lod0_vertices > 18_000);
        assert!(!pre.has_lod1_for_all_meshes);

        let (opt_bytes, rep) = optimize_glb_bytes(&raw, 3_000, 0.22, false).expect("optimize glb");
        assert!(!rep.skipped_already_optimized);
        assert!(rep.lod0_vertices <= 3_000, "got {}", rep.lod0_vertices);
        assert!(
            rep.lod1_vertices >= 350 && rep.lod1_vertices <= 900,
            "got {}",
            rep.lod1_vertices
        );

        let post = inspect_glb_bytes(&opt_bytes).expect("inspect post");
        assert!(post.has_lod1_for_all_meshes);
        assert!(post.has_color_0);
        assert!(post.has_turret);
        assert!(post.has_barrel);
        assert!(post.detected_sockets.iter().any(|(n, _)| n == "Muzzle_01"));

        // Second pass must be idempotent and return identical bytes
        let (opt_again, rep2) =
            optimize_glb_bytes(&opt_bytes, 3_000, 0.22, false).expect("idempotent pass");
        assert!(rep2.skipped_already_optimized);
        assert_eq!(opt_bytes, opt_again);
    }

    #[test]
    fn verifies_all_187_optimized_rust_ra2_glbs_under_vertex_budget_with_lod1_and_color0() {
        let workspace_models =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../rust-ra2/assets/models");
        if !workspace_models.is_dir() {
            return;
        }
        let mut glb_files = Vec::new();
        collect_glb_files_recursive(&workspace_models, &mut glb_files).unwrap();
        assert_eq!(
            glb_files.len(),
            187,
            "expected all 187 rust-ra2 .glb models on disk"
        );

        for p in &glb_files {
            let bytes = fs::read(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            assert!(
                bytes.len().is_multiple_of(4),
                "{} not 4-byte aligned",
                p.display()
            );
            let total_hdr_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
            assert_eq!(
                total_hdr_len,
                bytes.len(),
                "{} header len mismatch",
                p.display()
            );

            let info = inspect_glb_bytes(&bytes)
                .unwrap_or_else(|e| panic!("inspect {}: {e}", p.display()));
            assert!(
                info.lod0_vertices > 0 && info.lod0_vertices < 15_000,
                "{} LOD0 vertices {} exceeded 15,000 budget",
                p.display(),
                info.lod0_vertices
            );
            assert!(
                info.has_lod1_for_all_meshes && info.lod1_vertices > 0,
                "{} missing _LOD1 meshes",
                p.display()
            );
            assert!(
                info.has_color_0,
                "{} missing COLOR_0 vertex colors",
                p.display()
            );
        }

        let grizzly =
            inspect_glb_bytes(&fs::read(workspace_models.join("grizzly_tank.glb")).unwrap())
                .unwrap();
        assert!(
            grizzly.has_turret,
            "grizzly_tank.glb must preserve Turret node"
        );
    }
}
