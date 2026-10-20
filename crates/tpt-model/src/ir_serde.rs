//! Serde bridge between Rust values and the IR tree.
//!
//! Mapping:
//!
//! | Rust                           | IR                                  |
//! |--------------------------------|-------------------------------------|
//! | `bool`, integers, floats       | `Bool`, `Int`/`UInt`, `Float`       |
//! | `String`, `char`               | `Str`                               |
//! | `&[u8]`, `Vec<u8>`             | `Bytes`                             |
//! | `Option::None`, `()`           | `Null`                              |
//! | sequences, tuples              | `Seq`                               |
//! | structs, string-keyed maps     | `Map` (declaration order)           |
//! | unit enum variant              | `Str(variant)`                      |
//! | newtype / struct variant       | `Map([(variant, payload)])`         |
//!
//! This is serde's externally tagged enum layout, so `#[derive(Serialize)]` types round-trip.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use serde::de::Deserializer as _;
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};
use serde::ser::{self, Serialize};
use tpt_uir_stub::UirNode;

/// Error for both directions of the bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerdeError(pub String);

impl fmt::Display for SerdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// `core::error::Error` is the trait `std::error::Error` re-exports (Rust 1.81+). Implementing it
// unconditionally keeps `SerdeError` usable with serde's `std` feature, which dev-dependencies
// enable, while the crate stays `no_std`.
impl core::error::Error for SerdeError {}

impl ser::Error for SerdeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        SerdeError(msg.to_string())
    }
}

impl de::Error for SerdeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        SerdeError(msg.to_string())
    }
}

/// Serializes any `Serialize` value into an IR tree.
pub fn to_node<T: Serialize + ?Sized>(value: &T) -> Result<UirNode, SerdeError> {
    value.serialize(NodeSerializer)
}

/// Deserializes a Rust value from an IR tree.
pub fn from_node<T: DeserializeOwned>(node: &UirNode) -> Result<T, SerdeError> {
    T::deserialize(NodeDeserializer { node })
}

// ---- serialization --------------------------------------------------------

/// Serializer that produces a [`UirNode`].
#[derive(Debug, Clone, Copy)]
pub struct NodeSerializer;

/// Builds a `Seq`, optionally wrapped as the payload of a tuple variant.
pub struct SeqBuilder {
    items: Vec<UirNode>,
    variant: Option<&'static str>,
}

impl SeqBuilder {
    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerdeError> {
        self.items.push(value.serialize(NodeSerializer)?);
        Ok(())
    }

    fn finish(self) -> UirNode {
        wrap_variant(self.variant, UirNode::Seq(self.items))
    }
}

/// Builds a `Map`, optionally wrapped as the payload of a struct variant.
pub struct MapBuilder {
    entries: Vec<(String, UirNode)>,
    pending: Option<String>,
    variant: Option<&'static str>,
}

impl MapBuilder {
    fn finish(self) -> UirNode {
        wrap_variant(self.variant, UirNode::Map(self.entries))
    }
}

fn wrap_variant(variant: Option<&'static str>, payload: UirNode) -> UirNode {
    match variant {
        None => payload,
        Some(name) => UirNode::Map(alloc::vec![(name.to_string(), payload)]),
    }
}

fn key_string<T: Serialize + ?Sized>(key: &T) -> Result<String, SerdeError> {
    match key.serialize(NodeSerializer)? {
        UirNode::Str(s) => Ok(s),
        UirNode::Int(v) => Ok(v.to_string()),
        UirNode::UInt(v) => Ok(v.to_string()),
        UirNode::Bool(v) => Ok(v.to_string()),
        _ => Err(SerdeError("map keys must be strings".to_string())),
    }
}

impl ser::Serializer for NodeSerializer {
    type Ok = UirNode;
    type Error = SerdeError;
    type SerializeSeq = SeqBuilder;
    type SerializeTuple = SeqBuilder;
    type SerializeTupleStruct = SeqBuilder;
    type SerializeTupleVariant = SeqBuilder;
    type SerializeMap = MapBuilder;
    type SerializeStruct = MapBuilder;
    type SerializeStructVariant = MapBuilder;

    fn serialize_bool(self, v: bool) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<UirNode, SerdeError> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i16(self, v: i16) -> Result<UirNode, SerdeError> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i32(self, v: i32) -> Result<UirNode, SerdeError> {
        self.serialize_i64(v as i64)
    }
    fn serialize_i64(self, v: i64) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Int(v))
    }
    fn serialize_u8(self, v: u8) -> Result<UirNode, SerdeError> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u16(self, v: u16) -> Result<UirNode, SerdeError> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u32(self, v: u32) -> Result<UirNode, SerdeError> {
        self.serialize_u64(v as u64)
    }
    fn serialize_u64(self, v: u64) -> Result<UirNode, SerdeError> {
        Ok(UirNode::UInt(v))
    }
    fn serialize_i128(self, _: i128) -> Result<UirNode, SerdeError> {
        Err(SerdeError(
            "128-bit integers are not supported by the IR".to_string(),
        ))
    }
    fn serialize_u128(self, _: u128) -> Result<UirNode, SerdeError> {
        Err(SerdeError(
            "128-bit integers are not supported by the IR".to_string(),
        ))
    }
    fn serialize_f32(self, v: f32) -> Result<UirNode, SerdeError> {
        self.serialize_f64(v as f64)
    }
    fn serialize_f64(self, v: f64) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Float(v))
    }
    fn serialize_char(self, v: char) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Str(v.to_string()))
    }
    fn serialize_str(self, v: &str) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Str(v.to_string()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Bytes(v.to_vec()))
    }
    fn serialize_none(self) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Null)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<UirNode, SerdeError> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Null)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Null)
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<UirNode, SerdeError> {
        Ok(UirNode::Str(variant.to_string()))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<UirNode, SerdeError> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<UirNode, SerdeError> {
        Ok(wrap_variant(
            Some(variant),
            value.serialize(NodeSerializer)?,
        ))
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<SeqBuilder, SerdeError> {
        Ok(SeqBuilder {
            items: Vec::with_capacity(len.unwrap_or(0)),
            variant: None,
        })
    }
    fn serialize_tuple(self, len: usize) -> Result<SeqBuilder, SerdeError> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<SeqBuilder, SerdeError> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<SeqBuilder, SerdeError> {
        Ok(SeqBuilder {
            items: Vec::with_capacity(len),
            variant: Some(variant),
        })
    }
    fn serialize_map(self, len: Option<usize>) -> Result<MapBuilder, SerdeError> {
        Ok(MapBuilder {
            entries: Vec::with_capacity(len.unwrap_or(0)),
            pending: None,
            variant: None,
        })
    }
    fn serialize_struct(self, _: &'static str, len: usize) -> Result<MapBuilder, SerdeError> {
        self.serialize_map(Some(len))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<MapBuilder, SerdeError> {
        Ok(MapBuilder {
            entries: Vec::with_capacity(len),
            pending: None,
            variant: Some(variant),
        })
    }
}

impl ser::SerializeSeq for SeqBuilder {
    type Ok = UirNode;
    type Error = SerdeError;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerdeError> {
        self.push(value)
    }
    fn end(self) -> Result<UirNode, SerdeError> {
        Ok(self.finish())
    }
}

impl ser::SerializeTuple for SeqBuilder {
    type Ok = UirNode;
    type Error = SerdeError;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerdeError> {
        self.push(value)
    }
    fn end(self) -> Result<UirNode, SerdeError> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleStruct for SeqBuilder {
    type Ok = UirNode;
    type Error = SerdeError;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerdeError> {
        self.push(value)
    }
    fn end(self) -> Result<UirNode, SerdeError> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleVariant for SeqBuilder {
    type Ok = UirNode;
    type Error = SerdeError;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerdeError> {
        self.push(value)
    }
    fn end(self) -> Result<UirNode, SerdeError> {
        Ok(self.finish())
    }
}

impl ser::SerializeMap for MapBuilder {
    type Ok = UirNode;
    type Error = SerdeError;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), SerdeError> {
        self.pending = Some(key_string(key)?);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerdeError> {
        let key = self
            .pending
            .take()
            .ok_or_else(|| SerdeError("map value serialized before its key".to_string()))?;
        self.entries.push((key, value.serialize(NodeSerializer)?));
        Ok(())
    }
    fn end(self) -> Result<UirNode, SerdeError> {
        Ok(self.finish())
    }
}

impl ser::SerializeStruct for MapBuilder {
    type Ok = UirNode;
    type Error = SerdeError;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerdeError> {
        self.entries
            .push((key.to_string(), value.serialize(NodeSerializer)?));
        Ok(())
    }
    fn end(self) -> Result<UirNode, SerdeError> {
        Ok(self.finish())
    }
}

impl ser::SerializeStructVariant for MapBuilder {
    type Ok = UirNode;
    type Error = SerdeError;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerdeError> {
        self.entries
            .push((key.to_string(), value.serialize(NodeSerializer)?));
        Ok(())
    }
    fn end(self) -> Result<UirNode, SerdeError> {
        Ok(self.finish())
    }
}

// ---- deserialization ------------------------------------------------------

/// Deserializer that reads from a borrowed [`UirNode`].
#[derive(Debug, Clone, Copy)]
pub struct NodeDeserializer<'de> {
    node: &'de UirNode,
}

struct SeqDe<'de> {
    iter: core::slice::Iter<'de, UirNode>,
}

impl<'de> SeqAccess<'de> for SeqDe<'de> {
    type Error = SerdeError;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, SerdeError> {
        match self.iter.next() {
            Some(node) => seed.deserialize(NodeDeserializer { node }).map(Some),
            None => Ok(None),
        }
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

struct MapDe<'de> {
    iter: core::slice::Iter<'de, (String, UirNode)>,
    value: Option<&'de UirNode>,
}

impl<'de> MapAccess<'de> for MapDe<'de> {
    type Error = SerdeError;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, SerdeError> {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                let key: de::value::StrDeserializer<'de, SerdeError> =
                    key.as_str().into_deserializer();
                seed.deserialize(key).map(Some)
            }
            None => Ok(None),
        }
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, SerdeError> {
        let node = self
            .value
            .take()
            .ok_or_else(|| SerdeError("map value requested before its key".to_string()))?;
        seed.deserialize(NodeDeserializer { node })
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

struct EnumDe<'de> {
    variant: &'de str,
    value: Option<&'de UirNode>,
}

impl<'de> EnumAccess<'de> for EnumDe<'de> {
    type Error = SerdeError;
    type Variant = VariantDe<'de>;
    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, VariantDe<'de>), SerdeError> {
        let name: de::value::StrDeserializer<'de, SerdeError> = self.variant.into_deserializer();
        let value = seed.deserialize(name)?;
        Ok((value, VariantDe { value: self.value }))
    }
}

struct VariantDe<'de> {
    value: Option<&'de UirNode>,
}

impl<'de> VariantAccess<'de> for VariantDe<'de> {
    type Error = SerdeError;
    fn unit_variant(self) -> Result<(), SerdeError> {
        match self.value {
            None | Some(UirNode::Null) => Ok(()),
            Some(_) => Err(SerdeError("unit variant carries a payload".to_string())),
        }
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, SerdeError> {
        match self.value {
            Some(node) => seed.deserialize(NodeDeserializer { node }),
            None => Err(SerdeError(
                "newtype variant is missing its payload".to_string(),
            )),
        }
    }
    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, SerdeError> {
        match self.value {
            Some(node) => NodeDeserializer { node }.deserialize_seq(visitor),
            None => Err(SerdeError(
                "tuple variant is missing its payload".to_string(),
            )),
        }
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, SerdeError> {
        match self.value {
            Some(node) => NodeDeserializer { node }.deserialize_map(visitor),
            None => Err(SerdeError(
                "struct variant is missing its payload".to_string(),
            )),
        }
    }
}

impl<'de> de::Deserializer<'de> for NodeDeserializer<'de> {
    type Error = SerdeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, SerdeError> {
        match self.node {
            UirNode::Null => visitor.visit_unit(),
            UirNode::Bool(b) => visitor.visit_bool(*b),
            UirNode::Int(v) => visitor.visit_i64(*v),
            UirNode::UInt(v) => visitor.visit_u64(*v),
            UirNode::Float(v) => visitor.visit_f64(*v),
            UirNode::Str(s) => visitor.visit_borrowed_str(s.as_str()),
            UirNode::Bytes(b) => visitor.visit_borrowed_bytes(b.as_slice()),
            UirNode::Seq(items) => visitor.visit_seq(SeqDe { iter: items.iter() }),
            UirNode::Map(entries) => visitor.visit_map(MapDe {
                iter: entries.iter(),
                value: None,
            }),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, SerdeError> {
        match self.node {
            UirNode::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, SerdeError> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, SerdeError> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, SerdeError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, SerdeError> {
        match self.node {
            UirNode::Str(name) => visitor.visit_enum(EnumDe {
                variant: name.as_str(),
                value: None,
            }),
            UirNode::Map(entries) if entries.len() == 1 => {
                let (name, payload) = &entries[0];
                visitor.visit_enum(EnumDe {
                    variant: name.as_str(),
                    value: Some(payload),
                })
            }
            _ => Err(SerdeError(
                "expected a variant name or a single-entry map".to_string(),
            )),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        identifier ignored_any seq tuple tuple_struct map struct
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
        count: u32,
        ratio: f64,
        tags: Vec<String>,
        note: Option<String>,
        kind: Kind,
        pair: (u8, bool),
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Kind {
        Plain,
        Wrapped(i64),
        Detailed { level: u8 },
    }

    #[test]
    fn struct_round_trip() {
        let value = Sample {
            name: "bolt".into(),
            count: 7,
            ratio: 0.5,
            tags: vec!["a".into(), "b".into()],
            note: None,
            kind: Kind::Detailed { level: 3 },
            pair: (9, true),
        };
        let node = to_node(&value).unwrap();
        assert_eq!(node.get("name"), Some(&UirNode::Str("bolt".into())));
        assert_eq!(
            node.get("kind"),
            Some(&UirNode::Map(vec![(
                "Detailed".into(),
                UirNode::Map(vec![("level".into(), UirNode::UInt(3))])
            )]))
        );
        let back: Sample = from_node(&node).unwrap();
        assert_eq!(back, value);
    }

    #[test]
    fn unit_and_newtype_variants() {
        assert_eq!(to_node(&Kind::Plain).unwrap(), UirNode::Str("Plain".into()));
        let node = to_node(&Kind::Wrapped(-2)).unwrap();
        assert_eq!(from_node::<Kind>(&node).unwrap(), Kind::Wrapped(-2));
        assert_eq!(
            from_node::<Kind>(&UirNode::Str("Plain".into())).unwrap(),
            Kind::Plain
        );
    }

    #[test]
    fn wrong_shape_is_an_error_not_a_panic() {
        assert!(from_node::<Sample>(&UirNode::Int(3)).is_err());
        assert!(from_node::<Kind>(&UirNode::Str("Nope".into())).is_err());
    }
}
