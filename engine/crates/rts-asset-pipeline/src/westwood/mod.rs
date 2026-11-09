mod bundle_writer;
mod ini_parser;
mod mix_and_audio;
mod rules_converter;
mod sprite_relief;
mod sprite_shp;
mod sprite_turntable;
mod voxel_isosurface;
mod voxel_meshing;
mod voxel_vxl;

pub use bundle_writer::*;
pub use ini_parser::*;
pub use mix_and_audio::*;
pub use rules_converter::*;
use sprite_relief::*;
pub use sprite_shp::*;
pub use sprite_turntable::*;
use voxel_isosurface::*;
use voxel_meshing::*;
pub use voxel_vxl::*;


#[cfg(test)]
mod tests {
    use super::*;
    use rts_sdk::schema::ArmorType;
    use rts_sdk::loader::LoadedMod;
    use rts_sdk::validator::validate_mod;
    use std::fs;

    #[test]
    fn converts_westwood_rules_ini_into_valid_mod_bundle() {
        let sample_ini = r#"
; Sample Westwood Red Alert 2 rulesmd.ini snippet
[BuildingTypes]
0=GAPILE
1=GAPOWR
2=GAWEAP
3=NALASR

[InfantryTypes]
0=E1
1=ENGINEER

[VehicleTypes]
0=MTNK
1=HTNK
2=HARV

[GAPILE]
Name=Allied Barracks
Cost=500
Strength=800
Armor=steel
Power=-20
Owner=British,French,Germans,Americans
Factory=InfantryType
Foundation=2x2

[GAPOWR]
Name=Allied Power Plant
Cost=800
Strength=750
Armor=wood
Power=200
Owner=Allies
Foundation=2x2

[GAWEAP]
Name=Allied War Factory
Cost=2000
Strength=1000
Armor=steel
Power=-50
Prerequisite=GAPOWR
Owner=Allies,Soviet
Factory=UnitType
Foundation=3x3

[NALASR]
Name=Soviet Sentry Gun
Cost=600
Strength=600
Armor=steel
Power=-25
Primary=Vulcan2
Owner=Soviet
Foundation=1x1

[E1]
Name=GI
Cost=200
Strength=125
Armor=none
Speed=4
Sight=5
Primary=M60
Prerequisite=GAPILE
Owner=Allies

[ENGINEER]
Name=Engineer
Cost=500
Strength=75
Armor=none
Speed=4
Engineer=yes
Prerequisite=GAPILE
Owner=Allies,Soviet

[MTNK]
Name=Grizzly Battle Tank
Cost=700
Strength=400
Armor=heavy
Speed=7
Sight=8
Primary=105mm
Prerequisite=GAWEAP
Owner=Allies

[HTNK]
Name=Rhino Heavy Tank
Cost=900
Strength=550
Armor=heavy
Speed=6
Sight=8
Primary=120mm
Owner=Soviet

[HARV]
Name=War Miner
Cost=1400
Strength=1000
Armor=medium
Speed=5
Harvester=yes
Prerequisite=GAWEAP
Owner=Soviet

[M60]
Damage=25
ROF=20
Range=5
Warhead=SA

[105mm]
Damage=65
ROF=45
Range=6
Warhead=AP

[120mm]
Damage=90
ROF=55
Range=6.5
Warhead=AP

[Vulcan2]
Damage=40
ROF=25
Range=6
Warhead=SA

[SA]
Verses=100%,80%,60%,35%,25%

[AP]
Verses=40%,75%,100%,120%,85%
"#;

        let bundle =
            convert_westwood_ini_to_bundle(sample_ini, "westwood-conv", "Westwood Converted")
                .expect("should convert Westwood INI");
        assert_eq!(bundle.buildings.buildings.len(), 4);
        assert_eq!(bundle.units.units.len(), 5);
        assert_eq!(bundle.weapons.weapons.len(), 4);

        let mtnk = bundle
            .units
            .units
            .iter()
            .find(|u| u.id == "mtnk")
            .expect("mtnk should exist");
        assert_eq!(mtnk.name, "Grizzly Battle Tank");
        assert_eq!(mtnk.cost, 700);
        assert_eq!(mtnk.armor, ArmorType::Heavy);
        assert_eq!(mtnk.primary_weapon.as_deref(), Some("id_105mm"));
        assert_eq!(mtnk.prerequisites, vec!["gaweap".to_string()]);

        let tmp_dir = std::env::temp_dir().join("rts_test_westwood_conv");
        let _ = fs::remove_dir_all(&tmp_dir);
        write_converted_bundle_to_dir(&bundle, &tmp_dir, None).expect("should write converted mod");
        let loaded = LoadedMod::load_from_dir(&tmp_dir).expect("should load converted mod");
        let report = validate_mod(&loaded, true);
        assert!(
            report.is_valid(),
            "Converted Westwood mod should validate with 0 errors: {:?}",
            report.issues
        );
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn converts_westwood_vxl_voxel_bytes_to_valid_glb() {
        let vxl = build_sample_vxl_bytes(4, 6, 3);
        let glb = convert_vxl_bytes_to_glb(&vxl, None, "mtnk_voxel").expect("should convert vxl");
        assert_eq!(&glb[0..4], b"glTF");
        let version = u32::from_le_bytes(glb[4..8].try_into().unwrap());
        let total_len = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
        assert_eq!(version, 2);
        assert_eq!(total_len, glb.len());

        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json_val: serde_json::Value =
            serde_json::from_slice(&glb[20..20 + json_len]).expect("valid glTF JSON");
        assert!(
            json_val["meshes"][0]["primitives"][0]["attributes"]["COLOR_0"].is_number(),
            "Voxel GLB should include COLOR_0 vertex color attribute"
        );
    }

    #[test]
    fn parses_and_extracts_classic_westwood_mix_archive() {
        let payload = b"[MTNK]\nName=Medium Tank\nCost=800\nStrength=400\n";
        let crc = westwood_mix_crc("rules.ini");
        let mut mix = Vec::new();
        mix.extend_from_slice(&1u16.to_le_bytes()); // 1 file
        mix.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        mix.extend_from_slice(&crc.to_le_bytes());
        mix.extend_from_slice(&0u32.to_le_bytes()); // offset 0
        mix.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        mix.extend_from_slice(payload);

        let tmp_dir = std::env::temp_dir().join("rts_test_mix_extract");
        let _ = fs::remove_dir_all(&tmp_dir);
        let extracted = extract_mix_archive(&mix, &tmp_dir, &[]).expect("should extract mix");
        assert_eq!(extracted.len(), 1);
        assert_eq!(
            extracted[0].file_name().unwrap().to_str().unwrap(),
            "rules.ini"
        );
        let read_back = fs::read_to_string(&extracted[0]).unwrap();
        assert_eq!(read_back.as_bytes(), payload);
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn converts_shp_sprites_multipart_vxl_hva_dynamic_factions_and_rhai_traits() {
        // 1. Test 2D .shp sprite -> 3D .glb extrusion
        let shp = build_sample_shp_bytes(12, 16);
        let shp_glb = convert_shp_bytes_to_glb(&shp, None, "e1_sprite").expect("should convert shp");
        assert_eq!(&shp_glb[0..4], b"glTF");
        let total_len = u32::from_le_bytes(shp_glb[8..12].try_into().unwrap()) as usize;
        assert_eq!(total_len, shp_glb.len());

        // 2. Test multi-part .vxl (hull + turret + barrel) + .hva offset parsing
        let hull = build_sample_vxl_bytes(6, 8, 3);
        let tur = build_sample_vxl_bytes(4, 4, 2);
        let barl = build_sample_vxl_bytes(2, 6, 1);
        let single_glb = convert_vxl_bytes_to_glb(&hull, None, "mtnk").unwrap();
        let multi_glb = convert_multipart_vxl_to_glb(
            &[
                (&hull, [0.0, 0.0, 0.0]),
                (&tur, [0.0, 0.35, 0.0]),
                (&barl, [0.0, 0.42, 0.45]),
            ],
            None,
            "mtnk",
        )
        .expect("should combine hull+turret+barrel vxl");
        assert!(
            multi_glb.len() > single_glb.len(),
            "Combined hull+turret+barrel GLB should have more geometry than single hull"
        );

        // 3. Test dynamic multi-faction INI (GDI, Nod, Yuri) + SelfHealing Rhai hook + Westwood .map INI
        let multi_faction_ini = r#"
[Basic]
Name=Tiberian & Yuri Crossover

[Map]
Size=0,0,52,52

[Waypoints]
0=8008
1=42042

[BuildingTypes]
0=Gacnst
1=Nacnst
2=Yacnst

[InfantryTypes]
0=ENGI

[VehicleTypes]
0=MMK2

[Gacnst]
Name=GDI Construction Yard
Cost=2500
Strength=1500
ConstructionYard=yes
Owner=GDI
Foundation=3x3

[Nacnst]
Name=Temple of Nod Yard
Cost=2500
Strength=1500
ConstructionYard=yes
Owner=Nod
Foundation=3x3

[Yacnst]
Name=Yuri Citadel Yard
Cost=2500
Strength=1500
ConstructionYard=yes
Owner=Yuri
Foundation=3x3

[ENGI]
Name=Universal Engineer
Cost=500
Strength=100
Engineer=yes
Owner=GDI,Nod,Yuri

[MMK2]
Name=Mammoth Mk. II
Cost=2500
Strength=1200
Armor=heavy
Speed=5
SelfHealing=yes
Owner=GDI
"#;
        let bundle = convert_westwood_ini_to_bundle(
            multi_faction_ini,
            "multi-faction-test",
            "Multi Faction Test",
        )
        .expect("should convert multi-faction INI");

        let faction_ids: Vec<&str> = bundle
            .factions
            .factions
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert!(faction_ids.contains(&"gdi"));
        assert!(faction_ids.contains(&"nod"));
        assert!(faction_ids.contains(&"yuri"));
        assert_eq!(bundle.map.width, 52);
        assert_eq!(bundle.map.name, "Tiberian & Yuri Crossover");
        assert_eq!(
            bundle.manifest.scripts,
            vec!["scripts/auto_traits.rhai".to_string()]
        );

        let engi = bundle
            .units
            .units
            .iter()
            .find(|u| u.id == "engi")
            .expect("engi should exist");
        assert!(engi.is_available_to_faction("gdi"));
        assert!(engi.is_available_to_faction("nod"));
        assert!(engi.is_available_to_faction("yuri"));

        let mmk2 = bundle
            .units
            .units
            .iter()
            .find(|u| u.id == "mmk2")
            .expect("mmk2 should exist");
        assert_eq!(
            mmk2.script_hooks.get("on_tick").map(String::as_str),
            Some("on_self_heal_tick")
        );

        let tmp_dir = std::env::temp_dir().join("rts_test_westwood_multipart_shp");
        let _ = fs::remove_dir_all(&tmp_dir);
        write_converted_bundle_to_dir(&bundle, &tmp_dir, None).expect("should write bundle");
        let loaded = LoadedMod::load_from_dir(&tmp_dir).expect("should load mod with Rhai script");
        let report = validate_mod(&loaded, true);
        assert!(report.is_valid(), "Validation issues: {:?}", report.issues);
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn decodes_westwood_ima_adpcm_wav_and_audio_bag_to_pcm16_riff_wav() {
        // 1. Build a synthetic 1-block (64-byte) Westwood IMA ADPCM .wav (wFormatTag = 0x0011)
        let mut adpcm_block = vec![0u8; 64];
        adpcm_block[0..2].copy_from_slice(&1200i16.to_le_bytes()); // initial predictor
        adpcm_block[2] = 16; // initial step index
        for b in &mut adpcm_block[4..] {
            *b = 0x35; // alternating + and + nibbles
        }

        let mut ima_wav = Vec::new();
        let riff_len = 36 + adpcm_block.len() as u32;
        ima_wav.extend_from_slice(b"RIFF");
        ima_wav.extend_from_slice(&riff_len.to_le_bytes());
        ima_wav.extend_from_slice(b"WAVE");
        ima_wav.extend_from_slice(b"fmt ");
        ima_wav.extend_from_slice(&16u32.to_le_bytes());
        ima_wav.extend_from_slice(&0x0011u16.to_le_bytes()); // IMA ADPCM
        ima_wav.extend_from_slice(&1u16.to_le_bytes()); // 1 channel
        ima_wav.extend_from_slice(&22_050u32.to_le_bytes());
        ima_wav.extend_from_slice(&11_100u32.to_le_bytes());
        ima_wav.extend_from_slice(&64u16.to_le_bytes()); // block_align = 64
        ima_wav.extend_from_slice(&4u16.to_le_bytes()); // 4-bit ADPCM
        ima_wav.extend_from_slice(b"data");
        ima_wav.extend_from_slice(&(adpcm_block.len() as u32).to_le_bytes());
        ima_wav.extend_from_slice(&adpcm_block);

        let pcm_wav = convert_westwood_wav_to_pcm16(&ima_wav).expect("should decode IMA ADPCM wav");
        assert_eq!(&pcm_wav[0..4], b"RIFF");
        assert_eq!(&pcm_wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([pcm_wav[20], pcm_wav[21]]), 1); // PCM = 1
        assert_eq!(u16::from_le_bytes([pcm_wav[34], pcm_wav[35]]), 16); // 16-bit
        // 1 initial sample + 60 * 2 = 121 samples -> 242 data bytes + 44 header = 286 bytes
        assert_eq!(pcm_wav.len(), 44 + 121 * 2);

        // 2. Test audio.idx (GABA) + audio.bag extraction
        let mut idx = Vec::new();
        idx.extend_from_slice(b"GABA");
        idx.extend_from_slice(&2u32.to_le_bytes()); // version 2
        idx.extend_from_slice(&1u32.to_le_bytes()); // 1 entry
        let mut name_buf = [0u8; 16];
        name_buf[..8].copy_from_slice(b"vgriatta");
        idx.extend_from_slice(&name_buf);
        idx.extend_from_slice(&0u32.to_le_bytes()); // offset 0
        idx.extend_from_slice(&(adpcm_block.len() as u32).to_le_bytes());
        idx.extend_from_slice(&22_050u32.to_le_bytes());
        idx.extend_from_slice(&12u32.to_le_bytes()); // flags = 12 (mono IMA ADPCM)
        idx.extend_from_slice(&64u32.to_le_bytes()); // chunk_size = 64

        let entries = parse_westwood_audio_idx(&idx).expect("should parse audio.idx");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "vgriatta");

        let bag_wav = extract_westwood_bag_entry_to_pcm16_wav(&adpcm_block, &entries[0])
            .expect("should decode bag entry");
        assert_eq!(bag_wav, pcm_wav);
    }
}
