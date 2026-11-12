//! `RTSB01` binary deserializer (`Task 234`).

use super::{
    RtsbError, TAG_BOOL_FALSE, TAG_BOOL_TRUE, TAG_BYTES, TAG_ENUM_NEWTYPE, TAG_ENUM_SEQ,
    TAG_ENUM_UNIT, TAG_F32, TAG_F64, TAG_I16, TAG_I32, TAG_I64, TAG_I8, TAG_MAP, TAG_NONE_OR_UNIT,
    TAG_SEQ, TAG_SOME, TAG_STR, TAG_U16, TAG_U32, TAG_U64, TAG_U8,
};
use serde::de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};

pub(super) struct RtsbDeserializer<'de> {
    input: &'de [u8],
    pos: usize,
}

impl<'de> RtsbDeserializer<'de> {
    #[inline]
    pub(super) fn new(input: &'de [u8]) -> Self {
        Self { input, pos: 0 }
    }

    #[inline]
    pub(super) fn remaining_slice(&self) -> &'de [u8] {
        &self.input[self.pos..]
    }

    #[inline]
    fn read_u8(&mut self) -> Result<u8, RtsbError> {
        let b = *self
            .input
            .get(self.pos)
            .ok_or_else(|| RtsbError("RTSB01 unexpected EOF".to_string()))?;
        self.pos += 1;
        Ok(b)
    }

    #[inline]
    fn read_exact(&mut self, len: usize) -> Result<&'de [u8], RtsbError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| RtsbError("RTSB01 slice overflow".to_string()))?;
        if end > self.input.len() {
            return Err(RtsbError("RTSB01 unexpected EOF in payload".to_string()));
        }
        let s = &self.input[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    #[inline]
    fn read_u16(&mut self) -> Result<u16, RtsbError> {
        let s = self.read_exact(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    #[inline]
    fn read_u32(&mut self) -> Result<u32, RtsbError> {
        let s = self.read_exact(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    #[inline]
    fn read_u64(&mut self) -> Result<u64, RtsbError> {
        let s = self.read_exact(8)?;
        let mut arr = [0_u8; 8];
        arr.copy_from_slice(s);
        Ok(u64::from_le_bytes(arr))
    }

    #[inline]
    fn read_short_str(&mut self) -> Result<&'de str, RtsbError> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_exact(len)?;
        std::str::from_utf8(bytes)
            .map_err(|_| RtsbError("RTSB01 invalid UTF-8 identifier".to_string()))
    }

    #[inline]
    fn read_str(&mut self) -> Result<&'de str, RtsbError> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_exact(len)?;
        std::str::from_utf8(bytes).map_err(|_| RtsbError("RTSB01 invalid UTF-8 string".to_string()))
    }

    #[inline]
    fn read_bytes(&mut self) -> Result<&'de [u8], RtsbError> {
        let len = self.read_u32()? as usize;
        self.read_exact(len)
    }
}

impl<'de> de::Deserializer<'de> for &mut RtsbDeserializer<'de> {
    type Error = RtsbError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, RtsbError> {
        let tag = self.read_u8()?;
        match tag {
            TAG_NONE_OR_UNIT => visitor.visit_unit(),
            TAG_BOOL_FALSE => visitor.visit_bool(false),
            TAG_BOOL_TRUE => visitor.visit_bool(true),
            TAG_SOME => visitor.visit_some(self),
            TAG_U8 => visitor.visit_u8(self.read_u8()?),
            TAG_U16 => visitor.visit_u16(self.read_u16()?),
            TAG_U32 => visitor.visit_u32(self.read_u32()?),
            TAG_U64 => visitor.visit_u64(self.read_u64()?),
            TAG_I8 => visitor.visit_i8(self.read_u8()? as i8),
            TAG_I16 => visitor.visit_i16(self.read_u16()? as i16),
            TAG_I32 => visitor.visit_i32(self.read_u32()? as i32),
            TAG_I64 => visitor.visit_i64(self.read_u64()? as i64),
            TAG_F32 => visitor.visit_f32(f32::from_bits(self.read_u32()?)),
            TAG_F64 => visitor.visit_f64(f64::from_bits(self.read_u64()?)),
            TAG_STR => visitor.visit_borrowed_str(self.read_str()?),
            TAG_BYTES => visitor.visit_borrowed_bytes(self.read_bytes()?),
            TAG_SEQ => {
                let n = self.read_u32()? as usize;
                visitor.visit_seq(RtsbSeqAccess {
                    de: self,
                    remaining: n,
                })
            }
            TAG_MAP => {
                let n = self.read_u32()? as usize;
                visitor.visit_map(RtsbMapAccess {
                    de: self,
                    remaining: n,
                })
            }
            TAG_ENUM_UNIT => visitor.visit_borrowed_str(self.read_short_str()?),
            TAG_ENUM_NEWTYPE | TAG_ENUM_SEQ => {
                let variant = self.read_short_str()?;
                visitor.visit_map(RtsbSingleVariantMapAccess {
                    de: self,
                    variant: Some(variant),
                })
            }
            other => Err(RtsbError(format!("RTSB01 unknown value tag 0x{other:02x}"))),
        }
    }

    #[inline]
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, RtsbError> {
        let tag = self.read_u8()?;
        match tag {
            TAG_NONE_OR_UNIT => visitor.visit_none(),
            TAG_SOME => visitor.visit_some(self),
            _ => {
                self.pos -= 1;
                visitor.visit_some(self)
            }
        }
    }

    #[inline]
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, RtsbError> {
        visitor.visit_newtype_struct(self)
    }

    #[inline]
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, RtsbError> {
        let tag = self.read_u8()?;
        match tag {
            TAG_ENUM_UNIT | TAG_ENUM_NEWTYPE | TAG_ENUM_SEQ => {
                let variant = self.read_short_str()?;
                visitor.visit_enum(RtsbEnumAccess {
                    de: self,
                    tag,
                    variant,
                })
            }
            TAG_STR => {
                let variant = self.read_str()?;
                visitor.visit_enum(RtsbEnumAccess {
                    de: self,
                    tag: TAG_ENUM_UNIT,
                    variant,
                })
            }
            other => Err(RtsbError(format!(
                "RTSB01 expected enum tag, got 0x{other:02x}"
            ))),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }
}

struct RtsbSeqAccess<'a, 'de: 'a> {
    de: &'a mut RtsbDeserializer<'de>,
    remaining: usize,
}

impl<'de> SeqAccess<'de> for RtsbSeqAccess<'_, 'de> {
    type Error = RtsbError;

    #[inline]
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, RtsbError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }

    #[inline]
    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining.min(65_536))
    }
}

struct RtsbMapAccess<'a, 'de: 'a> {
    de: &'a mut RtsbDeserializer<'de>,
    remaining: usize,
}

impl<'de> MapAccess<'de> for RtsbMapAccess<'_, 'de> {
    type Error = RtsbError;

    #[inline]
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, RtsbError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }

    #[inline]
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, RtsbError> {
        seed.deserialize(&mut *self.de)
    }

    #[inline]
    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining.min(65_536))
    }
}

struct RtsbSingleVariantMapAccess<'a, 'de: 'a> {
    de: &'a mut RtsbDeserializer<'de>,
    variant: Option<&'de str>,
}

impl<'de> MapAccess<'de> for RtsbSingleVariantMapAccess<'_, 'de> {
    type Error = RtsbError;

    #[inline]
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, RtsbError> {
        let Some(v) = self.variant.take() else {
            return Ok(None);
        };
        let key_de: de::value::BorrowedStrDeserializer<'de, RtsbError> =
            de::value::BorrowedStrDeserializer::new(v);
        seed.deserialize(key_de).map(Some)
    }

    #[inline]
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, RtsbError> {
        seed.deserialize(&mut *self.de)
    }
}

struct RtsbEnumAccess<'a, 'de: 'a> {
    de: &'a mut RtsbDeserializer<'de>,
    tag: u8,
    variant: &'de str,
}

impl<'de> EnumAccess<'de> for RtsbEnumAccess<'_, 'de> {
    type Error = RtsbError;
    type Variant = Self;

    #[inline]
    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), RtsbError> {
        let val_de: de::value::BorrowedStrDeserializer<'de, RtsbError> =
            de::value::BorrowedStrDeserializer::new(self.variant);
        let v = seed.deserialize(val_de)?;
        Ok((v, self))
    }
}

impl<'de> VariantAccess<'de> for RtsbEnumAccess<'_, 'de> {
    type Error = RtsbError;

    #[inline]
    fn unit_variant(self) -> Result<(), RtsbError> {
        if self.tag != TAG_ENUM_UNIT {
            return Err(RtsbError("RTSB01 expected unit enum variant".to_string()));
        }
        Ok(())
    }

    #[inline]
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, RtsbError> {
        if self.tag != TAG_ENUM_NEWTYPE {
            return Err(RtsbError(
                "RTSB01 expected newtype enum variant".to_string(),
            ));
        }
        seed.deserialize(&mut *self.de)
    }

    #[inline]
    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, RtsbError> {
        if self.tag != TAG_ENUM_SEQ {
            return Err(RtsbError("RTSB01 expected tuple enum variant".to_string()));
        }
        de::Deserializer::deserialize_seq(&mut *self.de, visitor)
    }

    #[inline]
    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, RtsbError> {
        if self.tag != TAG_ENUM_SEQ {
            return Err(RtsbError("RTSB01 expected struct enum variant".to_string()));
        }
        de::Deserializer::deserialize_seq(&mut *self.de, visitor)
    }
}
