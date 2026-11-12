//! Deterministic `< 0.5 ms` binary world snapshot codec (`RTSB01`, `Task 234` / `Task 247`).
//!
//! Provides `SimWorld::encode_fast_binary(&self) -> Vec<u8>` and
//! `SimWorld::decode_fast_binary(&[u8]) -> Result<SimWorld, String>` (plus
//! `SimWorld::restore_fast_binary_in_place` for zero-rebuild HPA* cache reuse during rollback).

mod de;

use crate::pathfinding::GridHeightCache;
use crate::world::SimWorld;
use de::RtsbDeserializer;
use serde::ser::{
    self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// 6-byte magic header identifying an `RTSB01` binary world snapshot.
pub const RTSB01_MAGIC: &[u8; 6] = b"RTSB01";
/// Binary format version (`1`).
pub const RTSB01_VERSION: u16 = 1;
pub(super) const HEADER_LEN: usize = 16; // 6 magic + 2 version + 8 FNV-1a checksum

pub(super) const TAG_NONE_OR_UNIT: u8 = 0x00;
pub(super) const TAG_BOOL_FALSE: u8 = 0x01;
pub(super) const TAG_BOOL_TRUE: u8 = 0x02;
pub(super) const TAG_SOME: u8 = 0x03;
pub(super) const TAG_U8: u8 = 0x04;
pub(super) const TAG_U16: u8 = 0x05;
pub(super) const TAG_U32: u8 = 0x06;
pub(super) const TAG_U64: u8 = 0x07;
pub(super) const TAG_I8: u8 = 0x08;
pub(super) const TAG_I16: u8 = 0x09;
pub(super) const TAG_I32: u8 = 0x0A;
pub(super) const TAG_I64: u8 = 0x0B;
pub(super) const TAG_F32: u8 = 0x0C;
pub(super) const TAG_F64: u8 = 0x0D;
pub(super) const TAG_STR: u8 = 0x0E;
pub(super) const TAG_BYTES: u8 = 0x0F;
pub(super) const TAG_SEQ: u8 = 0x10;
pub(super) const TAG_MAP: u8 = 0x11;
pub(super) const TAG_ENUM_UNIT: u8 = 0x12;
pub(super) const TAG_ENUM_NEWTYPE: u8 = 0x13;
pub(super) const TAG_ENUM_SEQ: u8 = 0x14;

#[inline]
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    let (chunks, remainder) = bytes.as_chunks::<8>();
    for &chunk in chunks {
        let word = u64::from_le_bytes(chunk);
        hash ^= word;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for &b in remainder {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[derive(Debug, Clone)]
pub struct RtsbError(pub String);

impl fmt::Display for RtsbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RtsbError {}

impl ser::Error for RtsbError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

impl serde::de::Error for RtsbError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

struct RtsbSerializer {
    buf: Vec<u8>,
}

impl RtsbSerializer {
    #[inline]
    fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
        }
    }

    #[inline]
    fn write_short_str(&mut self, s: &str) -> Result<(), RtsbError> {
        let len = u16::try_from(s.len())
            .map_err(|_| RtsbError("RTSB01 enum identifier exceeds u16::MAX".to_string()))?;
        self.buf.extend_from_slice(&len.to_le_bytes());
        self.buf.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

struct RtsbCompound<'a> {
    ser: &'a mut RtsbSerializer,
    count_pos: usize,
    count: u32,
}

impl<'a> RtsbCompound<'a> {
    #[inline]
    fn begin(ser: &'a mut RtsbSerializer, tag: u8) -> Self {
        ser.buf.push(tag);
        let count_pos = ser.buf.len();
        ser.buf.extend_from_slice(&0_u32.to_le_bytes());
        Self {
            ser,
            count_pos,
            count: 0,
        }
    }

    #[inline]
    fn finish(self) -> Result<(), RtsbError> {
        let bytes = self.count.to_le_bytes();
        self.ser.buf[self.count_pos..self.count_pos + 4].copy_from_slice(&bytes);
        Ok(())
    }
}

impl<'a> ser::Serializer for &'a mut RtsbSerializer {
    type Ok = ();
    type Error = RtsbError;
    type SerializeSeq = RtsbCompound<'a>;
    type SerializeTuple = RtsbCompound<'a>;
    type SerializeTupleStruct = RtsbCompound<'a>;
    type SerializeTupleVariant = RtsbCompound<'a>;
    type SerializeMap = RtsbCompound<'a>;
    type SerializeStruct = RtsbCompound<'a>;
    type SerializeStructVariant = RtsbCompound<'a>;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<(), RtsbError> {
        self.buf
            .push(if v { TAG_BOOL_TRUE } else { TAG_BOOL_FALSE });
        Ok(())
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<(), RtsbError> {
        self.buf.push(TAG_I8);
        self.buf.push(v as u8);
        Ok(())
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<(), RtsbError> {
        self.buf.push(TAG_I16);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<(), RtsbError> {
        self.buf.push(TAG_I32);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<(), RtsbError> {
        self.buf.push(TAG_I64);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<(), RtsbError> {
        self.buf.push(TAG_U8);
        self.buf.push(v);
        Ok(())
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<(), RtsbError> {
        self.buf.push(TAG_U16);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<(), RtsbError> {
        self.buf.push(TAG_U32);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<(), RtsbError> {
        self.buf.push(TAG_U64);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<(), RtsbError> {
        self.buf.push(TAG_F32);
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
        Ok(())
    }

    #[inline]
    fn serialize_f64(self, v: f64) -> Result<(), RtsbError> {
        self.buf.push(TAG_F64);
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
        Ok(())
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<(), RtsbError> {
        let mut tmp = [0_u8; 4];
        self.serialize_str(v.encode_utf8(&mut tmp))
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<(), RtsbError> {
        self.buf.push(TAG_STR);
        self.buf.extend_from_slice(&(v.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(v.as_bytes());
        Ok(())
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<(), RtsbError> {
        self.buf.push(TAG_BYTES);
        self.buf.extend_from_slice(&(v.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(v);
        Ok(())
    }

    #[inline]
    fn serialize_none(self) -> Result<(), RtsbError> {
        self.buf.push(TAG_NONE_OR_UNIT);
        Ok(())
    }

    #[inline]
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), RtsbError> {
        self.buf.push(TAG_SOME);
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<(), RtsbError> {
        self.buf.push(TAG_NONE_OR_UNIT);
        Ok(())
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), RtsbError> {
        self.buf.push(TAG_NONE_OR_UNIT);
        Ok(())
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<(), RtsbError> {
        self.buf.push(TAG_ENUM_UNIT);
        self.write_short_str(variant)
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), RtsbError> {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), RtsbError> {
        self.buf.push(TAG_ENUM_NEWTYPE);
        self.write_short_str(variant)?;
        value.serialize(self)
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, RtsbError> {
        Ok(RtsbCompound::begin(self, TAG_SEQ))
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, RtsbError> {
        Ok(RtsbCompound::begin(self, TAG_SEQ))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, RtsbError> {
        Ok(RtsbCompound::begin(self, TAG_SEQ))
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, RtsbError> {
        self.buf.push(TAG_ENUM_SEQ);
        self.write_short_str(variant)?;
        Ok(RtsbCompound::begin(self, TAG_SEQ))
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, RtsbError> {
        Ok(RtsbCompound::begin(self, TAG_MAP))
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, RtsbError> {
        Ok(RtsbCompound::begin(self, TAG_SEQ))
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, RtsbError> {
        self.buf.push(TAG_ENUM_SEQ);
        self.write_short_str(variant)?;
        Ok(RtsbCompound::begin(self, TAG_SEQ))
    }
}

impl SerializeSeq for RtsbCompound<'_> {
    type Ok = ();
    type Error = RtsbError;
    #[inline]
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), RtsbError> {
        self.count = self.count.saturating_add(1);
        value.serialize(&mut *self.ser)
    }
    #[inline]
    fn end(self) -> Result<(), RtsbError> {
        self.finish()
    }
}

impl SerializeTuple for RtsbCompound<'_> {
    type Ok = ();
    type Error = RtsbError;
    #[inline]
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), RtsbError> {
        self.count = self.count.saturating_add(1);
        value.serialize(&mut *self.ser)
    }
    #[inline]
    fn end(self) -> Result<(), RtsbError> {
        self.finish()
    }
}

impl SerializeTupleStruct for RtsbCompound<'_> {
    type Ok = ();
    type Error = RtsbError;
    #[inline]
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), RtsbError> {
        self.count = self.count.saturating_add(1);
        value.serialize(&mut *self.ser)
    }
    #[inline]
    fn end(self) -> Result<(), RtsbError> {
        self.finish()
    }
}

impl SerializeTupleVariant for RtsbCompound<'_> {
    type Ok = ();
    type Error = RtsbError;
    #[inline]
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), RtsbError> {
        self.count = self.count.saturating_add(1);
        value.serialize(&mut *self.ser)
    }
    #[inline]
    fn end(self) -> Result<(), RtsbError> {
        self.finish()
    }
}

impl SerializeMap for RtsbCompound<'_> {
    type Ok = ();
    type Error = RtsbError;
    #[inline]
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), RtsbError> {
        self.count = self.count.saturating_add(1);
        key.serialize(&mut *self.ser)
    }
    #[inline]
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), RtsbError> {
        value.serialize(&mut *self.ser)
    }
    #[inline]
    fn end(self) -> Result<(), RtsbError> {
        self.finish()
    }
}

impl SerializeStruct for RtsbCompound<'_> {
    type Ok = ();
    type Error = RtsbError;
    #[inline]
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<(), RtsbError> {
        self.count = self.count.saturating_add(1);
        value.serialize(&mut *self.ser)
    }
    #[inline]
    fn end(self) -> Result<(), RtsbError> {
        self.finish()
    }
}

impl SerializeStructVariant for RtsbCompound<'_> {
    type Ok = ();
    type Error = RtsbError;
    #[inline]
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<(), RtsbError> {
        self.count = self.count.saturating_add(1);
        value.serialize(&mut *self.ser)
    }
    #[inline]
    fn end(self) -> Result<(), RtsbError> {
        self.finish()
    }
}

impl SimWorld {
    /// Serializes this `SimWorld` into a deterministic `RTSB01` binary snapshot (`< 0.5 ms` on a
    /// 400-entity world), including a 64-bit FNV-1a integrity checksum and preserved terrain
    /// height + active flow-field navigation caches.
    pub fn encode_fast_binary(&self) -> Vec<u8> {
        let est_cap =
            65_536 + self.entities.len() * 820 + self.grid.vertex_heights.len().saturating_mul(10);
        let mut ser = RtsbSerializer::with_capacity(est_cap);
        ser.buf.extend_from_slice(RTSB01_MAGIC);
        ser.buf.extend_from_slice(&RTSB01_VERSION.to_le_bytes());
        ser.buf.extend_from_slice(&0_u64.to_le_bytes()); // checksum placeholder

        self.serialize(&mut ser)
            .expect("SimWorld RTSB01 binary serialization is infallible");

        // Append transient height cache & warm flow-field navigation cache trailer
        if self.grid.has_synced_height_cache() {
            let hc = &self.grid.height_cache;
            ser.buf
                .reserve(13 + hc.tile_heights.len().saturating_mul(4));
            ser.buf.push(1);
            ser.buf
                .extend_from_slice(&hc.min_vertex_height.to_bits().to_le_bytes());
            ser.buf
                .extend_from_slice(&hc.max_vertex_height.to_bits().to_le_bytes());
            ser.buf
                .extend_from_slice(&(hc.tile_heights.len() as u32).to_le_bytes());
            for &h in &hc.tile_heights {
                ser.buf.extend_from_slice(&h.to_bits().to_le_bytes());
            }
        } else {
            ser.buf.push(0);
        }
        self.flow_field_cache.encode_binary_into(&mut ser.buf, 4);

        let checksum = fnv1a_64(&ser.buf[HEADER_LEN..]);
        ser.buf[8..16].copy_from_slice(&checksum.to_le_bytes());
        ser.buf
    }

    /// Restores a `SimWorld` from an `RTSB01` binary snapshot, validating the header magic,
    /// version, and 64-bit FNV-1a checksum and restoring/rebuilding navigation caches cleanly.
    pub fn decode_fast_binary(bytes: &[u8]) -> Result<SimWorld, String> {
        if bytes.len() < HEADER_LEN {
            return Err("RTSB01 snapshot buffer shorter than 16-byte header".to_string());
        }
        if &bytes[0..6] != RTSB01_MAGIC {
            return Err("RTSB01 header magic mismatch".to_string());
        }
        let version = u16::from_le_bytes([bytes[6], bytes[7]]);
        if version != RTSB01_VERSION {
            return Err(format!(
                "Unsupported RTSB01 snapshot version {version} (expected {RTSB01_VERSION})"
            ));
        }
        let mut csum_bytes = [0_u8; 8];
        csum_bytes.copy_from_slice(&bytes[8..16]);
        let expected_checksum = u64::from_le_bytes(csum_bytes);
        let actual_checksum = fnv1a_64(&bytes[HEADER_LEN..]);
        if actual_checksum != expected_checksum {
            return Err(format!(
                "RTSB01 checksum mismatch (expected 0x{expected_checksum:016x}, got 0x{actual_checksum:016x})"
            ));
        }

        let mut de = RtsbDeserializer::new(&bytes[HEADER_LEN..]);
        let mut world = SimWorld::deserialize(&mut de).map_err(|e| e.0)?;

        // Restore height cache & warm flow-field cache trailer
        let mut trailer = de.remaining_slice();
        if let Some((&has_hc, rest)) = trailer.split_first() {
            trailer = rest;
            if has_hc == 1 {
                if trailer.len() < 12 {
                    return Err("RTSB01 truncated in GridHeightCache trailer".to_string());
                }
                let min_vh = f32::from_bits(u32::from_le_bytes([
                    trailer[0], trailer[1], trailer[2], trailer[3],
                ]));
                let max_vh = f32::from_bits(u32::from_le_bytes([
                    trailer[4], trailer[5], trailer[6], trailer[7],
                ]));
                let count =
                    u32::from_le_bytes([trailer[8], trailer[9], trailer[10], trailer[11]]) as usize;
                trailer = &trailer[12..];
                let byte_len = count.saturating_mul(4);
                if count > 1_048_576 || trailer.len() < byte_len {
                    return Err("RTSB01 invalid GridHeightCache length".to_string());
                }
                let mut tile_heights = Vec::with_capacity(count.min(65_536));
                for &chunk in trailer[..byte_len].as_chunks::<4>().0 {
                    tile_heights.push(f32::from_bits(u32::from_le_bytes(chunk)));
                }
                trailer = &trailer[byte_len..];
                world.grid.height_cache = GridHeightCache {
                    tile_heights,
                    min_vertex_height: min_vh,
                    max_vertex_height: max_vh,
                    synced_vertex_len: world.grid.vertex_heights.len(),
                    synced_width: world.grid.width,
                    synced_height: world.grid.height,
                };
            } else if world.grid.has_valid_vertex_heights() {
                world.grid.rebuild_height_cache();
            }
            if !trailer.is_empty() {
                world.flow_field_cache.decode_binary_from(&mut trailer)?;
            }
        } else if world.grid.has_valid_vertex_heights() {
            world.grid.rebuild_height_cache();
        }

        let expected_cells =
            (world.grid.width.max(0) as usize).saturating_mul(world.grid.height.max(0) as usize);
        if expected_cells > 0 && world.grid.clearance_map.len() != expected_cells {
            world.grid.rebuild_clearance_map();
        }
        world.power_dirty = true;
        Ok(world)
    }

    /// Restores `self` from an `RTSB01` snapshot while retaining existing `hpa_cache` sector graphs
    /// whenever `grid.topology_epoch` matches across the rollback boundary.
    pub fn restore_fast_binary_in_place(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut prev_hpa = std::mem::take(&mut self.hpa_cache);
        let prev_epoch = self.grid.topology_epoch;
        let mut restored = Self::decode_fast_binary(bytes)?;
        if restored.grid.topology_epoch == prev_epoch {
            restored.hpa_cache = prev_hpa;
        } else {
            prev_hpa.clear();
        }
        *self = restored;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sdk::loader::LoadedMod;
    use std::path::PathBuf;
    use std::time::Instant;

    fn load_ra2_for_codec_test() -> LoadedMod {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../rust-ra2");
        LoadedMod::load_from_dir(&root).expect("rust-ra2 should load")
    }

    #[test]
    fn rtsb01_roundtrip_preserves_exact_state_hash_and_explored_tiles_on_400_entities() {
        let loaded = load_ra2_for_codec_test();
        let mut world = SimWorld::from_mod(&loaded);

        // Spawn units up to >= 400 living entities and run 10 ticks to warm caches & triggers
        let mut idx = 0_i32;
        while world.entities.len() < 405 {
            let pid = if idx % 2 == 0 { 1 } else { 2 };
            let uid = if pid == 1 {
                "allied_gi"
            } else {
                "soviet_conscript"
            };
            let tx = 8 + (idx % 40);
            let tz = 8 + (idx / 40) * 2;
            world.spawn_unit(pid, uid, [tx, tz], &loaded);
            idx += 1;
        }
        for _ in 0..10 {
            world.step(&loaded);
        }

        let expected_hash = world.state_hash();
        let expected_sub = world.subsystem_hashes();
        assert_ne!(expected_sub.explored_tiles, 0);

        let snap = world.encode_fast_binary();
        assert_eq!(&snap[0..6], RTSB01_MAGIC);

        let decoded = SimWorld::decode_fast_binary(&snap).expect("RTSB01 decode must succeed");
        assert_eq!(decoded.state_hash(), expected_hash);
        assert_eq!(decoded.subsystem_hashes(), expected_sub);

        // Verify corrupt byte is caught by 64-bit FNV-1a checksum
        let mut corrupted = snap.clone();
        let mid = corrupted.len() / 2;
        corrupted[mid] ^= 0x5A;
        assert!(SimWorld::decode_fast_binary(&corrupted).is_err());

        // Verify deterministic stepping parity after restore_fast_binary_in_place
        let mut clone_a = world.clone();
        let mut clone_b = SimWorld::from_mod(&loaded);
        clone_b
            .restore_fast_binary_in_place(&snap)
            .expect("in-place restore must succeed");
        for _ in 0..5 {
            clone_a.step(&loaded);
            clone_b.step(&loaded);
        }
        assert_eq!(clone_a.state_hash(), clone_b.state_hash());

        if !cfg!(debug_assertions) {
            let iters = 20_u32;
            let mut last_bytes = world.encode_fast_binary(); // 1 warm-up pass
            let mut enc_samples = Vec::with_capacity(iters as usize);
            for _ in 0..iters {
                let t0 = Instant::now();
                last_bytes = world.encode_fast_binary();
                enc_samples.push(t0.elapsed().as_secs_f64() * 1000.0);
            }
            enc_samples.sort_by(|a, b| a.total_cmp(b));
            let med_enc_ms = enc_samples[(iters as usize) / 2];

            let mut dec_samples = Vec::with_capacity(iters as usize);
            for _ in 0..iters {
                let t1 = Instant::now();
                let w = SimWorld::decode_fast_binary(&last_bytes).unwrap();
                std::hint::black_box(w.tick);
                dec_samples.push(t1.elapsed().as_secs_f64() * 1000.0);
            }
            dec_samples.sort_by(|a, b| a.total_cmp(b));
            let med_dec_ms = dec_samples[(iters as usize) / 2];
            println!(
                "[RTSB01 Benchmark] 400-entity snapshot ({} KB): median encode={med_enc_ms:.3}ms, decode={med_dec_ms:.3}ms",
                last_bytes.len() / 1024
            );
            assert!(
                med_enc_ms < 0.50,
                "RTSB01 400-entity encode exceeded 0.50ms: encode={med_enc_ms:.3}ms, decode={med_dec_ms:.3}ms"
            );
        }
    }
}
