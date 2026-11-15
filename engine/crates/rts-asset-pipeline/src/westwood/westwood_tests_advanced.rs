use super::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use std::fs;

#[test]
fn decodes_westwood_wsadpcm_aud_to_pcm16_riff_wav() {
    // Waveform exercising 2-bit small steps, 4-bit medium steps, 6-bit deltas, and large literal jumps
    let mut raw_u8 = Vec::with_capacity(64);
    let mut s = 128i32;
    for step in [
        -2, -1, 0, 1, // 2-bit table
        -9, 8, -5, 4, // 4-bit table
        -24, 21, -15, 19, // 6-bit signed delta
        90, -140, 75, -60, // literal fallback jumps
    ]
    .into_iter()
    .cycle()
    .take(64)
    {
        s = (s + step).clamp(8, 248);
        raw_u8.push(s as u8);
    }

    let aud_bytes = encode_ws_adpcm_aud(&raw_u8, 22_050, false);
    assert_eq!(
        u32::from_le_bytes(aud_bytes[16..20].try_into().unwrap()),
        WESTWOOD_AUD_CHUNK_MAGIC
    );

    let (hdr, samples) = parse_westwood_aud(&aud_bytes).expect("should parse WSADPCM .aud");
    assert_eq!(hdr.sample_rate, 22_050);
    assert_eq!(hdr.compression_type, 1);
    assert_eq!(samples.len(), raw_u8.len());
    for (idx, (&orig_u8, &dec_i16)) in raw_u8.iter().zip(samples.iter()).enumerate() {
        let expected_i16 = ((orig_u8 as i32 - 128) * 256) as i16;
        assert_eq!(
            dec_i16, expected_i16,
            "WSADPCM sample {idx} mismatch: got {dec_i16}, expected {expected_i16}"
        );
    }

    let wav = convert_westwood_aud_to_pcm16_wav(&aud_bytes).expect("should convert .aud to .wav");
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1); // PCM
    assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16); // 16-bit
    assert_eq!(wav.len(), 44 + raw_u8.len() * 2);

    // Verify auto-detection inside convert_westwood_wav_to_pcm16
    let auto_wav = convert_westwood_wav_to_pcm16(&aud_bytes).unwrap();
    assert_eq!(auto_wav, wav);
}

#[test]
fn decrypts_blowfish_ecb_mix_archives_and_resolves_xcc_and_known_filenames() {
    // 1. Official Bruce Schneier Blowfish test vectors
    let bf_zero = BlowfishCipher::new(&[0u8; 8]).unwrap();
    let mut blk0 = [0u8; 8];
    bf_zero.encrypt_block_be(&mut blk0);
    assert_eq!(blk0, [0x4E, 0xF9, 0x97, 0x45, 0x61, 0x98, 0xDD, 0x78]);
    bf_zero.decrypt_block_be(&mut blk0);
    assert_eq!(blk0, [0u8; 8]);

    let bf_ff = BlowfishCipher::new(&[0xFFu8; 8]).unwrap();
    let mut blk_ff = [0xFFu8; 8];
    bf_ff.encrypt_block_be(&mut blk_ff);
    assert_eq!(blk_ff, [0x51, 0x86, 0x6F, 0xD5, 0xB8, 0x5E, 0xCB, 0x8A]);
    bf_ff.decrypt_block_be(&mut blk_ff);
    assert_eq!(blk_ff, [0xFFu8; 8]);

    // 2. Verify expanded Westwood filename dictionary (350+ entries)
    assert!(
        known_westwood_filenames().len() >= 350,
        "Expected >= 350 known Westwood filenames, got {}",
        known_westwood_filenames().len()
    );

    // 3. Build an XCC `local mix database.dat` + RSA/Blowfish encrypted RA2 `.mix` (`flags = 0x00020000`)
    let mut xcc_db = Vec::new();
    xcc_db.extend_from_slice(b"XCC by Olaf van der Spek\x1a\x04\x17\x27\x10\x19\x80\x00");
    xcc_db.resize(52, 0);
    xcc_db[48..52].copy_from_slice(&2u32.to_le_bytes());
    xcc_db.extend_from_slice(b"custom_ra2_mission.map\0");
    xcc_db.extend_from_slice(b"secret_unit.vxl\0");

    let parsed_xcc = parse_xcc_local_mix_database(&xcc_db);
    assert!(parsed_xcc.contains(&"custom_ra2_mission.map".to_string()));
    assert!(parsed_xcc.contains(&"secret_unit.vxl".to_string()));

    let mut bf_key = [0u8; 56];
    for (i, b) in bf_key.iter_mut().enumerate() {
        *b = ((i * 37 + 19) & 0xFF) as u8;
    }
    let rules_payload = b"[MTNK]\nName=Grizzly\nCost=700\nStrength=400\n";
    let mission_payload = b"[Basic]\nName=Encrypted Mission\n[Map]\nSize=0,0,40,40\n";

    let encrypted_mix = build_encrypted_mix_archive(
        &[
            ("rulesmd.ini", rules_payload),
            ("local mix database.dat", &xcc_db),
            ("custom_ra2_mission.map", mission_payload),
        ],
        &bf_key,
    )
    .expect("should build RSA + Blowfish encrypted .mix");

    // Verify flags word has 0x0002_0000 set
    let flags = u32::from_le_bytes(encrypted_mix[0..4].try_into().unwrap());
    assert_ne!(flags & 0x0002_0000, 0);

    let tmp_dir = std::env::temp_dir().join("rts_test_encrypted_mix");
    let _ = fs::remove_dir_all(&tmp_dir);
    let extracted = extract_mix_archive(&encrypted_mix, &tmp_dir, &[])
        .expect("should decrypt MIX via 80-byte RSA key_source + Blowfish ECB");
    assert_eq!(extracted.len(), 3);
    assert_eq!(
        fs::read(tmp_dir.join("rulesmd.ini")).unwrap(),
        rules_payload
    );
    assert_eq!(
        fs::read(tmp_dir.join("custom_ra2_mission.map")).unwrap(),
        mission_payload
    );

    // Also verify explicit 56-byte Blowfish key path
    let (_, explicit_entries) =
        parse_mix_archive_with_key(&encrypted_mix, &[], Some(&bf_key)).unwrap();
    assert_eq!(explicit_entries.len(), 3);
    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn decompresses_format80_lcw_and_converts_td_ra1_shp_and_cameo_png() {
    let mut raw_pattern = Vec::with_capacity(512);
    raw_pattern.extend_from_slice(&[0x24; 48]); // RLE repeat run
    for i in 0..128u8 {
        raw_pattern.push((i.wrapping_mul(7)) ^ 0x5A); // Literal sequence
    }
    let repeat_slice = raw_pattern[32..112].to_vec();
    raw_pattern.extend_from_slice(&repeat_slice); // Back-reference copy

    let compressed = compress_format80(&raw_pattern);
    let decompressed =
        decompress_format80(&compressed, raw_pattern.len()).expect("Format80 LCW roundtrip");
    assert_eq!(decompressed, raw_pattern);

    let td_shp = build_sample_td_format80_shp_bytes(24, 24, 2);
    let glb = convert_shp_bytes_to_glb(&td_shp, None, "e1").expect("TD Format80 SHP -> GLB");
    assert_eq!(&glb[0..4], b"glTF");

    let png = convert_shp_cameo_to_png(&td_shp, None, Some([0.2, 0.5, 0.9]), "e1icon")
        .expect("TD Format80 SHP -> PNG cameo");
    assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn embeds_multiframe_hva_keyframe_animations_into_vxl_glb() {
    let hull_vxl = build_sample_vxl_bytes(6, 8, 3);
    let tur_vxl = build_sample_vxl_bytes(4, 4, 2);
    let hull_hva = build_sample_multiframe_hva_bytes(&["BODY"], 6);
    let tur_hva = build_sample_multiframe_hva_bytes(&["TURRET"], 6);

    let parsed_hva = parse_hva_animation(&hull_hva).expect("should parse multi-frame .hva");
    assert_eq!(parsed_hva.num_frames, 6);
    assert_eq!(parsed_hva.num_sections, 1);

    let anim_glb = convert_multipart_vxl_with_hva_to_glb(
        &[
            (&hull_vxl, Some(&hull_hva), [0.0, 0.0, 0.0]),
            (&tur_vxl, Some(&tur_hva), [0.0, 0.35, 0.0]),
        ],
        None,
        Some([0.2, 0.5, 0.9]),
        "mtnk",
    )
    .expect("should build animated multipart GLB");
    assert_eq!(&anim_glb[0..4], b"glTF");

    let json_len = u32::from_le_bytes(anim_glb[12..16].try_into().unwrap()) as usize;
    let gltf_json: serde_json::Value =
        serde_json::from_slice(&anim_glb[20..20 + json_len]).expect("valid glTF 2.0 JSON");
    let anims = gltf_json["animations"]
        .as_array()
        .expect("glTF animations array must exist when num_frames > 1");
    assert!(!anims.is_empty());
    let channels = anims[0]["channels"].as_array().unwrap();
    let samplers = anims[0]["samplers"].as_array().unwrap();
    assert!(
        channels.len() >= 4,
        "Hull + Turret should emit translation + rotation channels (got {})",
        channels.len()
    );
    assert_eq!(channels.len(), samplers.len());
    let paths: Vec<&str> = channels
        .iter()
        .filter_map(|c| c["target"]["path"].as_str())
        .collect();
    assert!(paths.contains(&"translation"));
    assert!(paths.contains(&"rotation"));
}

#[test]
fn decodes_isomappack5_overlaypack_triggers_and_links_art_and_sound_ini() {
    // Build synthetic 32x32 IsoMapPack5 records (water, cliff/elevation, ramp, clear ground)
    let mut records = Vec::new();
    for y in 0..32i16 {
        for x in 0..32i16 {
            let (tile_num, z_height) = if (16..=19).contains(&x) && (16..=19).contains(&y) {
                (62u32, 0u8) // Water tile away from spawns (59..=82)
            } else if (20..=22).contains(&x) && (10..=12).contains(&y) {
                (160u32, 1u8) // Ramp tile (z_height = base_z + 1, 150..=189)
            } else if (23..=25).contains(&x) && (10..=12).contains(&y) {
                (5u32, 4u8) // Elevated plateau (z_height >= base_z + 2)
            } else {
                (1u32, 0u8) // Clear ground
            };
            records.push(IsoMapTileRecord {
                rx: x,
                ry: y,
                tile_num,
                sub_tile: 0,
                z_height,
                ice_growth: 0,
            });
        }
    }
    let iso_lines = encode_westwood_format80_pack_ini_lines(&encode_isomappack5_records(&records));

    // Build 262,144-byte OverlayPack & OverlayDataPack (Ore=30, Gems=42, Bridge=24, Crate=26, Tree=75)
    let mut ovl_types = vec![0xFFu8; 262_144];
    let mut ovl_data = vec![0u8; 262_144];
    let set_ovl = |types: &mut [u8], data: &mut [u8], x: usize, y: usize, id: u8, dens: u8| {
        let idx = y * 512 + x;
        types[idx] = id;
        data[idx] = dens;
    };
    for dx in 12..=14 {
        set_ovl(&mut ovl_types, &mut ovl_data, dx, 12, 30, 8); // Riparius Ore
        set_ovl(&mut ovl_types, &mut ovl_data, dx, 24, 42, 10); // Cruentus Gems
    }
    set_ovl(&mut ovl_types, &mut ovl_data, 17, 17, 24, 0); // Bridge over water
    set_ovl(&mut ovl_types, &mut ovl_data, 14, 15, 26, 0); // Crate
    set_ovl(&mut ovl_types, &mut ovl_data, 15, 22, 75, 0); // Tree

    let ovl_lines = encode_westwood_format80_pack_ini_lines(&ovl_types);
    let ovl_data_lines = encode_westwood_format80_pack_ini_lines(&ovl_data);

    let rules_and_map_ini = format!(
        r#"
[Basic]
Name=IsoPack Valley

[Map]
Size=0,0,32,32
Theater=TEMPERATE

[Waypoints]
0=6006
1=26026
2=14014

[IsoMapPack5]
{iso_lines}
[OverlayPack]
{ovl_lines}
[OverlayDataPack]
{ovl_data_lines}
[Tags]
01000001=0,ReinforceTag,01000000

[CellTags]
14014=01000001

[Triggers]
01000000=Americans,<none>,AlliedDrop,0,1,1,1,0

[Events]
01000000=1,1,0,1

[Actions]
01000000=2,11,4,EVA_Reinforcements,0,0,0,0,A,105,0,1500,0,0,0,0,A

[BuildingTypes]
0=GACNST
1=GAWEAP

[VehicleTypes]
0=MTNK
1=HARV

[GACNST]
Name=Allied Construction Yard
Cost=2500
Strength=1500
ConstructionYard=yes
Owner=Allies
Foundation=3x3

[GAWEAP]
Name=Allied War Factory
Cost=2000
Strength=1000
Factory=UnitType
Owner=Allies,Soviet
Foundation=3x3

[MTNK]
Name=Grizzly Tank
Image=MTNK
Cost=700
Strength=400
Armor=heavy
Speed=7
Primary=105mm
VoiceSelect=GenTankSelect
VoiceMove=GenTankMove
VoiceAttack=GenTankAttack
DieSound=GenVehicleDie
Owner=Allies

[HARV]
Name=War Miner
Cost=1400
Strength=1000
Armor=medium
Speed=5
Harvester=yes
Owner=Soviet

[105mm]
Damage=65
ROF=45
Range=6
Report=Tank105Fire
Warhead=AP

[AP]
Verses=40%,75%,100%,120%,85%
"#
    );

    let art_ini = r#"
[GACNST]
Cameo=CNSTICON
Voxel=no

[MTNK]
Cameo=MTNKICON
Voxel=yes
"#;

    let sound_ini = r#"
[GenTankSelect]
Sounds= $vgrisele $vgriselb

[GenTankMove]
Sounds= $vgrimova

[GenTankAttack]
Sounds= $vgriatta

[GenVehicleDie]
Sounds= bgendiea

[Tank105Fire]
Sounds= btank105
"#;

    let eva_ini = r#"
[DialogList]
0=EVA_UnitReady
1=EVA_LowPower

[EVA_UnitReady]
Allied=ceva001
Russian=seva001

[EVA_LowPower]
Allied=ceva002
Russian=seva002
"#;

    let bundle = convert_westwood_ini_files_to_bundle(
        &rules_and_map_ini,
        Some(art_ini),
        Some(sound_ini),
        Some(eva_ini),
        "isopack-suite-mod",
        "IsoPack Suite Mod",
    )
    .expect("should convert rules + map + art + sound + eva");

    assert!(
        bundle.map.heightmap.is_some(),
        "heightmap must be populated"
    );
    assert!(bundle.map.splatmap.is_some(), "splatmap must be populated");
    assert!(
        !bundle.map.water_rects.is_empty(),
        "water_rects must be decoded"
    );
    assert!(
        !bundle.map.elevation_rects.is_empty(),
        "elevation_rects must be decoded"
    );
    assert!(
        !bundle.map.ramp_rects.is_empty(),
        "ramp_rects must be decoded"
    );
    assert!(
        !bundle.map.bridge_rects.is_empty(),
        "bridge_rects must be decoded"
    );
    assert!(
        !bundle.map.ore_patches.is_empty(),
        "ore_patches must be decoded"
    );
    assert!(
        !bundle.map.gem_patches.is_empty(),
        "gem_patches must be decoded"
    );
    assert!(
        !bundle.map.trigger_zones.is_empty(),
        "trigger_zones must be decoded"
    );

    let mtnk = bundle.units.units.iter().find(|u| u.id == "mtnk").unwrap();
    assert_eq!(
        mtnk.visual.icon.as_deref(),
        Some("assets/icons/mtnkicon.png")
    );

    let audio = bundle
        .audio
        .as_ref()
        .expect("audio catalog must be populated");
    assert_eq!(
        audio.weapon_sfx.get("id_105mm").map(String::as_str),
        Some("assets/audio/btank105.wav")
    );
    assert!(audio.unit_voices.contains_key("mtnk"));
    assert!(audio.eva_cues.contains_key("unit_ready"));

    // Write bundle with synthetic .csf, .aud, and cameo .shp in vxl_dir and validate with 0 errors!
    let tmp_root = std::env::temp_dir().join("rts_test_isopack_art_sound_mod");
    let _ = fs::remove_dir_all(&tmp_root);
    let stage_assets = tmp_root.join("raw_assets");
    let out_mod = tmp_root.join("cooked_mod");
    fs::create_dir_all(&stage_assets).unwrap();

    let csf_bytes = encode_westwood_csf(&CsfStringTable {
        version: 3,
        language_id: 0,
        language_code: "en".to_string(),
        entries: vec![CsfStringEntry {
            label: "Name:MTNK".to_string(),
            value: "Grizzly Battle Tank".to_string(),
            extra_value: None,
        }],
    });
    fs::write(stage_assets.join("ra2md.csf"), csf_bytes).unwrap();
    let aud_bytes = encode_ws_adpcm_aud(&[128, 130, 134, 126, 128, 120, 128, 136], 22_050, false);
    fs::write(stage_assets.join("btank105.aud"), aud_bytes).unwrap();
    let cameo_shp = build_sample_td_format80_shp_bytes(60, 48, 1);
    fs::write(stage_assets.join("mtnkicon.shp"), cameo_shp).unwrap();

    write_converted_bundle_to_dir(&bundle, &out_mod, Some(&stage_assets))
        .expect("write_converted_bundle_to_dir should succeed");
    assert!(out_mod.join("locales/en.toml").is_file());
    assert!(out_mod.join("audio/audio.toml").is_file());
    assert!(out_mod.join("assets/icons/mtnkicon.png").is_file());
    assert!(out_mod.join("assets/audio/btank105.wav").is_file());

    let loaded = LoadedMod::load_from_dir(&out_mod).expect("LoadedMod::load_from_dir");
    let val = validate_mod(&loaded, true);
    assert!(
        val.is_valid(),
        "Full Westwood suite mod must validate with 0 errors: {:?}",
        val.issues
    );
    let _ = fs::remove_dir_all(&tmp_root);
}

pub(super) fn build_authored_test_vxl(
    sx: u8,
    sy: u8,
    sz: u8,
    normals_mode: u8,
    mut voxel_fn: impl FnMut(u8, u8, u8) -> Option<(u8, u8)>,
) -> Vec<u8> {
    let mut vxl = Vec::new();
    let mut magic = [0u8; 16];
    magic[..15].copy_from_slice(b"Voxel Animation");
    vxl.extend_from_slice(&magic);
    vxl.extend_from_slice(&1u32.to_le_bytes());
    vxl.extend_from_slice(&1u32.to_le_bytes());
    vxl.extend_from_slice(&1u32.to_le_bytes());

    let num_cols = (sx as usize) * (sy as usize);
    let mut col_blobs: Vec<Vec<u8>> = Vec::with_capacity(num_cols);
    for y in 0..sy {
        for x in 0..sx {
            let mut packet = Vec::new();
            let mut z = 0u8;
            while z < sz {
                let skip_start = z;
                while z < sz && voxel_fn(x, y, z).is_none() {
                    z += 1;
                }
                let skip = z - skip_start;
                let mut run = Vec::new();
                while z < sz {
                    if let Some(cn) = voxel_fn(x, y, z) {
                        run.push(cn);
                        z += 1;
                    } else {
                        break;
                    }
                }
                if !run.is_empty() || skip > 0 {
                    packet.push(skip);
                    packet.push(run.len() as u8);
                    for (c, n) in &run {
                        packet.push(*c);
                        packet.push(*n);
                    }
                    packet.push(run.len() as u8);
                }
            }
            col_blobs.push(packet);
        }
    }

    let span_start_size = num_cols * 4;
    let span_end_size = num_cols * 4;
    let span_data_size: usize = col_blobs.iter().map(Vec::len).sum();
    let body_size = (span_start_size + span_end_size + span_data_size) as u32;
    vxl.extend_from_slice(&body_size.to_le_bytes());
    vxl.extend_from_slice(&[16u8, 31u8]);

    // Distinct 6-bit VGA palette entries outside 16..=31
    for i in 0..256u16 {
        let (r, g, b) = match i {
            4 => (8, 10, 12),
            12 => (16, 18, 22),
            48 => (58, 12, 8),
            140 => (10, 52, 22),
            220 => (14, 38, 60),
            _ => (
                ((i * 17 + 5) % 64) as u8,
                ((i * 31 + 11) % 64) as u8,
                ((i * 43 + 19) % 64) as u8,
            ),
        };
        vxl.extend_from_slice(&[r, g, b]);
    }

    let mut limb_name = [0u8; 16];
    limb_name[..4].copy_from_slice(b"BODY");
    vxl.extend_from_slice(&limb_name);
    vxl.extend_from_slice(&0u32.to_le_bytes());
    vxl.extend_from_slice(&1u32.to_le_bytes());
    vxl.extend_from_slice(&0u32.to_le_bytes());

    let mut cur = 0i32;
    for blob in &col_blobs {
        if blob.is_empty() {
            vxl.extend_from_slice(&(-1i32).to_le_bytes());
        } else {
            vxl.extend_from_slice(&cur.to_le_bytes());
            cur += blob.len() as i32;
        }
    }
    cur = 0;
    for blob in &col_blobs {
        if blob.is_empty() {
            vxl.extend_from_slice(&(-1i32).to_le_bytes());
        } else {
            let end = cur + blob.len() as i32 - 1;
            vxl.extend_from_slice(&end.to_le_bytes());
            cur += blob.len() as i32;
        }
    }
    for blob in &col_blobs {
        vxl.extend_from_slice(blob);
    }

    vxl.extend_from_slice(&0u32.to_le_bytes());
    vxl.extend_from_slice(&(span_start_size as u32).to_le_bytes());
    vxl.extend_from_slice(&((span_start_size + span_end_size) as u32).to_le_bytes());
    vxl.extend_from_slice(&0.25f32.to_le_bytes());
    for i in 0..12 {
        let v = if i == 0 || i == 5 || i == 10 {
            1.0f32
        } else {
            0.0f32
        };
        vxl.extend_from_slice(&v.to_le_bytes());
    }
    for _ in 0..6 {
        vxl.extend_from_slice(&0.0f32.to_le_bytes());
    }
    vxl.extend_from_slice(&[sx, sy, sz, normals_mode]);
    vxl
}

#[test]
fn decodes_ra2_244_and_ts_36_voxel_normals_with_35_deg_crease_splitting() {
    assert_eq!(WESTWOOD_TS_NORMALS_36.len(), 36);
    assert_eq!(WESTWOOD_RA2_NORMALS_244.len(), 244);
    assert!(decode_westwood_voxel_normal_gltf(2, 36).is_none());
    assert!(decode_westwood_voxel_normal_gltf(4, 244).is_none());

    // RA2 index 232 is Westwood `[0.040639, 0.024465, 0.998874]` -> glTF `[y, z, x]` (~+Y up)
    let up_ra2 = decode_westwood_voxel_normal_gltf(4, 232).unwrap();
    assert!(
        up_ra2[1] > 0.99,
        "RA2 +Z normal must map to glTF +Y, got {up_ra2:?}"
    );
    let up_ts = decode_westwood_voxel_normal_gltf(2, 32).unwrap();
    assert!(
        up_ts[1] > 0.99,
        "TS +Z normal must map to glTF +Y, got {up_ts:?}"
    );

    // 1. Hard-surface armor box (`> 35°` across edges): normals must stay planar with zero ballooning
    let box_vxl = build_authored_test_vxl(6, 6, 4, 4, |_x, _y, z| {
        Some((48, if z == 3 { 232 } else { 16 }))
    });
    let box_glb = convert_vxl_bytes_to_glb(&box_vxl, None, "armor_box").unwrap();
    let box_tris = crate::glb_optimizer::extract_world_triangles_from_glb(&box_glb).unwrap();
    assert!(!box_tris.is_empty());
    for tri in &box_tris {
        for n in &tri.n {
            let max_axis = n[0].abs().max(n[1].abs()).max(n[2].abs());
            assert!(
                max_axis > 0.99,
                "Hard-surface 90-deg box edge must split cleanly (>35 deg), got {n:?}"
            );
        }
    }

    // 2. Curved dome/barrel (`<= 35°` between adjacent RA2 normal indices 232 and 214: ~14° apart)
    let dome_vxl = build_authored_test_vxl(4, 4, 2, 4, |_x, y, _z| {
        Some((20, if y < 2 { 232 } else { 214 }))
    });
    let dome_glb = convert_vxl_bytes_to_glb(&dome_vxl, None, "kirov_dome").unwrap();
    let dome_tris = crate::glb_optimizer::extract_world_triangles_from_glb(&dome_glb).unwrap();
    let has_interpolated = dome_tris
        .iter()
        .flat_map(|t| t.n.iter())
        .any(|n| n[1] > 0.95 && n[0] > 0.10 && n[0] < 0.20);
    assert!(
        has_interpolated,
        "Adjacent Westwood normals <= 35 deg must smoothly interpolate"
    );
}

#[test]
fn encodes_lossless_png_uv_atlas_texcoord0_and_exact_16_to_31_alpha_team_mask() {
    let vxl = build_authored_test_vxl(8, 6, 4, 4, |x, y, z| {
        let c_idx = if z == 3 {
            if x < 4 {
                16
            } else {
                31
            }
        } else if y == 0 {
            4
        } else if y == 5 {
            48
        } else {
            22
        };
        Some((c_idx, 232))
    });

    let glb = convert_vxl_bytes_to_glb(&vxl, None, "grizzly_tank").expect("convert textured vxl");
    let (json, bin) = crate::glb_optimizer::parse_glb_chunks(&glb).expect("parse glb");

    let mat = &json["materials"][0];
    assert_eq!(mat["alphaMode"], "OPAQUE");
    assert_eq!(mat["pbrMetallicRoughness"]["baseColorTexture"]["index"], 0);
    assert!(
        (mat["pbrMetallicRoughness"]["metallicFactor"]
            .as_f64()
            .unwrap()
            - 0.32)
            .abs()
            < 1e-5
    );
    assert!(
        (mat["pbrMetallicRoughness"]["roughnessFactor"]
            .as_f64()
            .unwrap()
            - 0.58)
            .abs()
            < 1e-5
    );
    assert_eq!(json["samplers"][0]["magFilter"], 9728);
    assert_eq!(json["images"][0]["mimeType"], "image/png");

    let attrs = &json["meshes"][0]["primitives"][0]["attributes"];
    assert!(attrs["POSITION"].is_number() && attrs["NORMAL"].is_number());
    assert!(attrs["COLOR_0"].is_number() && attrs["TEXCOORD_0"].is_number());

    let (w, h, rgba) = super::westwood_tests_core::decode_glb_embedded_png(&json, bin);
    assert!(w >= 16 && h >= 16);

    let tris = crate::glb_optimizer::extract_world_triangles_from_glb(&glb).unwrap();
    let (mut saw_bright_remap, mut saw_dark_remap) = (false, false);
    let (mut saw_non_remap_tread, mut saw_non_remap_trim) = (false, false);

    for tri in &tris {
        let a0 = tri.rgba[0][3];
        assert!(
            (a0 == 0.0 && tri.rgba[1][3] == 0.0 && tri.rgba[2][3] == 0.0)
                || (a0 == 1.0 && tri.rgba[1][3] == 1.0 && tri.rgba[2][3] == 1.0),
            "Zero alpha bleed allowed across remap/non-remap quads, got {:?}",
            tri.rgba
        );
        if a0 == 0.0 {
            for c in &tri.rgba {
                assert!((c[0] - c[1]).abs() < 1e-5 && (c[1] - c[2]).abs() < 1e-5);
                assert!(
                    c[0] >= 0.179 && c[0] <= 1.0,
                    "Remap lum out of [0.18, 1.0]: {}",
                    c[0]
                );
                saw_bright_remap |= c[0] > 0.85;
                saw_dark_remap |= c[0] < 0.25;
            }
        } else {
            for c in &tri.rgba {
                saw_non_remap_trim |= c[0] > 0.65 && c[1] < 0.25;
                saw_non_remap_tread |= c[0] < 0.25 && c[2] < 0.25;
            }
        }
    }
    assert!(
        saw_bright_remap && saw_dark_remap,
        "Must encode 16..=31 grayscale ramp with Alpha=0"
    );
    assert!(
        saw_non_remap_tread && saw_non_remap_trim,
        "Must preserve 0..15 & 32..255 with Alpha=255"
    );
    assert!(rgba
        .chunks_exact(4)
        .any(|px| px[3] == 0 && px[0] == px[1] && px[1] == px[2]));
    assert!(rgba.chunks_exact(4).any(|px| px[3] == 255));

    let tinted = convert_multipart_vxl_to_glb_with_team_color(
        &[(&vxl, [0.0, 0.0, 0.0])],
        None,
        Some([0.85, 0.15, 0.12]),
        "soviet_tank",
    )
    .unwrap();
    let tinted_tris = crate::glb_optimizer::extract_world_triangles_from_glb(&tinted).unwrap();
    assert!(tinted_tris
        .iter()
        .any(|t| t.rgba[0][3] == 0.0 && t.rgba[0][0] > t.rgba[0][2]));
}
