//! Protocol buffer binary payload decoding.
//!
//! The decoding functionality can be accessed by building a decoding context and acquiring a
//! message or message reference. See the example in the [crate root](crate).

use crate::context::*;
use bytes::{Bytes, BytesMut};
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;

impl Context {
    /// Decode a message.
    pub fn decode(&self, msg: MessageRef, data: &[u8]) -> MessageValue {
        self.resolve_message(msg).decode(data, self)
    }
}

/// Decoded protocol buffer value.
#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    /// `double` value.
    Double(f64),
    /// `float` value.
    Float(f32),
    /// `int32` value.
    Int32(i32),
    /// `int64` value.
    Int64(i64),
    /// `uint32` value.
    UInt32(u32),
    /// `uint64` value.
    UInt64(u64),
    /// `sint32` value.
    SInt32(i32),
    /// `sint64` value.
    SInt64(i64),
    /// `fixed32` value.
    Fixed32(u32),
    /// `fixed64` value.
    Fixed64(u64),
    /// `sfixed32` value.
    SFixed32(i32),
    /// `sfixed64` value.
    SFixed64(i64),
    /// `bool` value.
    Bool(bool),
    /// `string` value.
    String(String),
    /// `bytes` value.
    Bytes(Bytes),

    /// A repeated packed value.
    Packed(PackedArray),

    /// Message type value.
    Message(Box<MessageValue>),

    /// Enum type value.
    Enum(EnumValue),

    /// Value which was incomplete due to missing bytes in the payload.
    Incomplete(u8, Bytes),

    /// Value which wasn't defined in the context.
    ///
    /// The wire type allows the decoder to tell how large an unknown value is. This allows the
    /// unknown value to be skipped and decoding can continue from the next value.
    Unknown(UnknownValue),
}

/// Packed scalar fields.
#[derive(Debug, PartialEq, Clone)]
pub enum PackedArray {
    /// `double` value.
    Double(Vec<f64>),
    /// `float` value.
    Float(Vec<f32>),
    /// `int32` value.
    Int32(Vec<i32>),
    /// `int64` value.
    Int64(Vec<i64>),
    /// `uint32` value.
    UInt32(Vec<u32>),
    /// `uint64` value.
    UInt64(Vec<u64>),
    /// `sint32` value.
    SInt32(Vec<i32>),
    /// `sint64` value.
    SInt64(Vec<i64>),
    /// `fixed32` value.
    Fixed32(Vec<u32>),
    /// `fixed64` value.
    Fixed64(Vec<u64>),
    /// `sfixed32` value.
    SFixed32(Vec<i32>),
    /// `sfixed64` value.
    SFixed64(Vec<i64>),
    /// `bool` value.
    Bool(Vec<bool>),
}

/// Unknown value.
#[derive(Debug, PartialEq, Clone)]
pub enum UnknownValue {
    /// Unknown varint (wire type = 0).
    Varint(u128),

    /// Unknown 64-bit value (wire type = 1).
    Fixed64(u64),

    /// Unknown variable length value (wire type = 2).
    VariableLength(Bytes),

    /// Unknown 32-bit value (wire type = 5).
    Fixed32(u32),

    /// Invalid value.
    ///
    /// Invalid value is a value for which the wire type wasn't valid. Encountering invalid wire
    /// type will result in the remaining bytes to be consumed from the current variable length
    /// stream as it is imposible to tell how large such invalid value is.
    ///
    /// The decoding will continue after the current variable length value.
    Invalid(u8, Bytes),
}

/// Enum value.
#[derive(Debug, PartialEq, Clone)]
pub struct EnumValue {
    /// Reference to the enum type.
    pub enum_ref: EnumRef,

    /// Value.
    pub value: i64,
}

/// Message value.
#[derive(Debug, PartialEq, Clone)]
pub struct MessageValue {
    /// Reference to the message type.
    pub msg_ref: MessageRef,

    /// Mesage field values.
    pub fields: Vec<FieldValue>,

    /// Garbage data at the end of the message.
    ///
    /// As opposed to an `UnknownValue::Invalid`, the garbage data did not have a valid field
    /// number and for that reason cannot be placed into the `fields` vector.
    pub garbage: Option<Bytes>,
}

/// Field value.
#[derive(Debug, PartialEq, Clone)]
pub struct FieldValue {
    /// Field number.
    pub number: u64,

    /// Field value.
    pub value: Value,
}

impl Value {
    fn decode(data: &mut &[u8], vt_raw: u8, vt: &ValueType, ctx: &Context) -> Self {
        let original = *data;
        let opt = match vt {
            ValueType::Double => {
                try_read_8_bytes(data).map(|b| Value::Double(f64::from_le_bytes(b)))
            }
            ValueType::Float => try_read_4_bytes(data).map(|b| Value::Float(f32::from_le_bytes(b))),
            ValueType::Int32 => i32::from_signed_varint(data).map(Value::Int32),
            ValueType::Int64 => i64::from_signed_varint(data).map(Value::Int64),
            ValueType::UInt32 => u32::from_unsigned_varint(data).map(Value::UInt32),
            ValueType::UInt64 => u64::from_unsigned_varint(data).map(Value::UInt64),
            ValueType::SInt32 => u32::from_unsigned_varint(data).map(|u| {
                let (sign, sign_bit) = if u % 2 == 0 { (1i32, 0) } else { (-1i32, 1) };
                let magnitude = (u / 2) as i32 + sign_bit;
                Value::SInt32(sign * magnitude)
            }),
            ValueType::SInt64 => u64::from_unsigned_varint(data).map(|u| {
                let (sign, sign_bit) = if u % 2 == 0 { (1i64, 0) } else { (-1i64, 1) };
                let magnitude = (u / 2) as i64 + sign_bit;
                Value::SInt64(sign * magnitude)
            }),
            ValueType::Fixed32 => {
                try_read_4_bytes(data).map(|b| Value::Fixed32(u32::from_le_bytes(b)))
            }
            ValueType::Fixed64 => {
                try_read_8_bytes(data).map(|b| Value::Fixed64(u64::from_le_bytes(b)))
            }
            ValueType::SFixed32 => {
                try_read_4_bytes(data).map(|b| Value::SFixed32(i32::from_le_bytes(b)))
            }
            ValueType::SFixed64 => {
                try_read_8_bytes(data).map(|b| Value::SFixed64(i64::from_le_bytes(b)))
            }
            ValueType::Bool => usize::from_unsigned_varint(data).map(|u| Value::Bool(u != 0)),
            ValueType::String => read_string(data).map(Value::String),
            ValueType::Bytes => read_bytes(data).map(Value::Bytes),
            ValueType::Enum(eref) => i64::from_signed_varint(data).map(|v| {
                Value::Enum(EnumValue {
                    enum_ref: *eref,
                    value: v,
                })
            }),
            ValueType::Message(mref) => usize::from_unsigned_varint(data).and_then(|length| {
                if data.len() < length {
                    *data = original;
                    return None;
                }
                let (consumed, remainder) = data.split_at(length);
                *data = remainder;

                Some(Value::Message(Box::new(
                    ctx.resolve_message(*mref).decode(consumed, ctx),
                )))
            }),
        };

        opt.unwrap_or_else(|| {
            *data = &[];
            Value::Incomplete(vt_raw, Bytes::copy_from_slice(original))
        })
    }

    fn decode_packed(data: &mut &[u8], vt_raw: u8, vt: &ValueType) -> Self {
        let original = *data;
        let length = match usize::from_unsigned_varint(data) {
            Some(len) => len,
            None => {
                return return_incomplete(data, vt_raw, original);
            }
        };

        if data.len() < length {
            return return_incomplete(data, vt_raw, original);
        }

        let mut array = &data[..length];
        *data = &data[length..];

        // Reading the packed arrays follows very similar format for each type.  The variances are
        // in how to read the data from the stream and what to do with the data to get the final
        // value.
        //
        // This macro implements the basic structure with holes for the varying bits.
        macro_rules! read_packed {
            ($variant:ident @ $val:ident = $try_read:expr => $insert:expr ) => {
                let mut output = vec![];
                loop {
                    if array.is_empty() {
                        break Value::Packed(PackedArray::$variant(output));
                    }

                    match $try_read {
                        Some($val) => output.push($insert),
                        None => return return_incomplete(&mut array, vt_raw, original),
                    }
                }
            };
        }

        match vt {
            ValueType::Double => {
                read_packed! { Double @ b = try_read_8_bytes(&mut array) => f64::from_le_bytes(b) }
            }
            ValueType::Float => {
                read_packed! { Float @ b = try_read_4_bytes(&mut array) => f32::from_le_bytes(b) }
            }
            ValueType::Int32 => {
                read_packed! { Int32 @ b = i32::from_signed_varint(&mut array) => b }
            }
            ValueType::Int64 => {
                read_packed! { Int64 @ b = i64::from_signed_varint(&mut array) => b }
            }
            ValueType::UInt32 => {
                read_packed! { UInt32 @ b = u32::from_signed_varint(&mut array) => b }
            }
            ValueType::UInt64 => {
                read_packed! { UInt64 @ b = u64::from_signed_varint(&mut array) => b }
            }
            ValueType::SInt32 => {
                read_packed! { SInt32 @ b = u32::from_signed_varint(&mut array) => {
                    let (sign, sign_bit) = if b % 2 == 0 { (1i32, 0) } else { (-1i32, 1) };
                    let magnitude = (b / 2) as i32 + sign_bit;
                    sign * magnitude
                } }
            }
            ValueType::SInt64 => {
                read_packed! { SInt64 @ b = u64::from_signed_varint(&mut array) => {
                    let (sign, sign_bit) = if b % 2 == 0 { (1i64, 0) } else { (-1i64, 1) };
                    let magnitude = (b / 2) as i64 + sign_bit;
                    sign * magnitude
                } }
            }
            ValueType::Fixed32 => {
                read_packed! { Fixed32 @ b = try_read_4_bytes(&mut array) => u32::from_le_bytes(b) }
            }
            ValueType::Fixed64 => {
                read_packed! { Fixed64 @ b = try_read_8_bytes(&mut array) => u64::from_le_bytes(b) }
            }
            ValueType::SFixed32 => {
                read_packed! { SFixed32 @ b = try_read_4_bytes(&mut array) => i32::from_le_bytes(b) }
            }
            ValueType::SFixed64 => {
                read_packed! { SFixed64 @ b = try_read_8_bytes(&mut array) => i64::from_le_bytes(b) }
            }
            ValueType::Bool => {
                read_packed! { Bool @ b = u8::from_unsigned_varint(&mut array) => b != 0 }
            }
            _ => panic!("Non-scalar type was handled as packed"),
        }
    }

    fn decode_unknown(data: &mut &[u8], vt: u8) -> Value {
        let original = *data;
        let value =
            match vt {
                0 => u128::from_unsigned_varint(data).map(UnknownValue::Varint),
                1 => try_read_8_bytes(data)
                    .map(|value| UnknownValue::Fixed64(u64::from_le_bytes(value))),
                2 => usize::from_unsigned_varint(data).and_then(|length| {
                    if length > data.len() {
                        *data = original;
                        return None;
                    }
                    let (consumed, remainder) = data.split_at(length);
                    *data = remainder;
                    Some(UnknownValue::VariableLength(Bytes::copy_from_slice(
                        consumed,
                    )))
                }),
                5 => try_read_4_bytes(data)
                    .map(|value| UnknownValue::Fixed32(u32::from_le_bytes(value))),
                _ => {
                    let bytes = Bytes::copy_from_slice(data);
                    *data = &[];
                    Some(UnknownValue::Invalid(vt, bytes))
                }
            };

        value
            .map(Value::Unknown)
            .unwrap_or_else(|| Value::Incomplete(vt, Bytes::copy_from_slice(data)))
    }

    fn encode(&self, ctx: &Context) -> Option<(u8, BytesMut)> {
        let bytes = match self {
            Value::Double(v) => BytesMut::from(v.to_le_bytes().as_ref()),
            Value::Float(v) => BytesMut::from(v.to_le_bytes().as_ref()),
            Value::Int32(v) => BytesMut::from(v.into_signed_varint().as_ref()),
            Value::Int64(v) => BytesMut::from(v.into_signed_varint().as_ref()),
            Value::UInt32(v) => BytesMut::from(v.into_unsigned_varint().as_ref()),
            Value::UInt64(v) => BytesMut::from(v.into_unsigned_varint().as_ref()),
            Value::SInt32(v) => {
                let (v, sign_bit) = if *v < 0 { (-v, 1) } else { (*v, 0) };
                (v * 2 - sign_bit).into_unsigned_varint()
            }
            Value::SInt64(v) => {
                let (v, sign_bit) = if *v < 0 { (-v, 1) } else { (*v, 0) };
                (v * 2 - sign_bit).into_unsigned_varint()
            }
            Value::Fixed32(v) => BytesMut::from(v.to_le_bytes().as_ref()),
            Value::Fixed64(v) => BytesMut::from(v.to_le_bytes().as_ref()),
            Value::SFixed32(v) => BytesMut::from(v.to_le_bytes().as_ref()),
            Value::SFixed64(v) => BytesMut::from(v.to_le_bytes().as_ref()),
            Value::Bool(v) => BytesMut::from(if *v { [1u8].as_ref() } else { [0u8].as_ref() }),
            Value::String(v) => {
                let mut output = v.len().into_unsigned_varint();
                output.extend_from_slice(v.as_bytes());
                output
            }
            Value::Bytes(v) => {
                let mut output = v.len().into_unsigned_varint();
                output.extend_from_slice(v);
                output
            }
            Value::Enum(v) => BytesMut::from(v.value.into_signed_varint().as_ref()),
            Value::Message(v) => {
                let data = v.encode(ctx);
                let mut output = data.len().into_unsigned_varint();
                output.extend_from_slice(&data);
                output
            }
            Value::Packed(p) => p.encode(),
            Value::Unknown(u) => u.encode(),
            Value::Incomplete(_, bytes) => BytesMut::from(bytes.as_ref()),
        };

        Some((self.wire_type(), bytes))
    }

    fn wire_type(&self) -> u8 {
        match self {
            Value::Double(..) => 1,
            Value::Float(..) => 5,
            Value::Int32(..) => 0,
            Value::Int64(..) => 0,
            Value::UInt32(..) => 0,
            Value::UInt64(..) => 0,
            Value::SInt32(..) => 0,
            Value::SInt64(..) => 0,
            Value::Fixed32(..) => 5,
            Value::Fixed64(..) => 1,
            Value::SFixed32(..) => 5,
            Value::SFixed64(..) => 1,
            Value::Bool(..) => 0,
            Value::String(..) => 2,
            Value::Bytes(..) => 2,
            Value::Message(..) => 2,
            Value::Enum(..) => 0,
            Value::Packed(..) => 2,
            Value::Unknown(unk) => match unk {
                UnknownValue::Varint(..) => 0,
                UnknownValue::Fixed64(..) => 1,
                UnknownValue::VariableLength(..) => 2,
                UnknownValue::Fixed32(..) => 5,
                UnknownValue::Invalid(vt, ..) => *vt,
            },
            Value::Incomplete(vt, ..) => *vt,
        }
    }
}

impl PackedArray {
    fn encode(&self) -> BytesMut {
        macro_rules! write_packed {
            ($value:ident => $convert:expr ) => {
                $value.iter().flat_map($convert).collect()
            };
        }

        let data: Bytes = match self {
            PackedArray::Double(v) => {
                write_packed!(v => |v| BytesMut::from(v.to_le_bytes().as_ref()))
            }
            PackedArray::Float(v) => {
                write_packed!(v => |v| BytesMut::from(v.to_le_bytes().as_ref()))
            }
            PackedArray::Int32(v) => {
                write_packed!(v => |v| BytesMut::from(v.into_signed_varint().as_ref()))
            }
            PackedArray::Int64(v) => {
                write_packed!(v => |v| BytesMut::from(v.into_signed_varint().as_ref()))
            }
            PackedArray::UInt32(v) => {
                write_packed!(v => |v| BytesMut::from(v.into_unsigned_varint().as_ref()))
            }
            PackedArray::UInt64(v) => {
                write_packed!(v => |v| BytesMut::from(v.into_unsigned_varint().as_ref()))
            }
            PackedArray::SInt32(v) => {
                write_packed! { v => |v| {
                    let (v, sign_bit) = if *v < 0 { (-v, 1) } else { (*v, 0) };
                    (v * 2 - sign_bit).into_unsigned_varint()
                } }
            }
            PackedArray::SInt64(v) => {
                write_packed! { v => |v| {
                    let (v, sign_bit) = if *v < 0 { (-v, 1) } else { (*v, 0) };
                    (v * 2 - sign_bit).into_unsigned_varint()
                } }
            }
            PackedArray::Fixed32(v) => {
                write_packed!( v => |v| BytesMut::from(v.to_le_bytes().as_ref()) )
            }
            PackedArray::Fixed64(v) => {
                write_packed!( v => |v| BytesMut::from(v.to_le_bytes().as_ref()) )
            }
            PackedArray::SFixed32(v) => {
                write_packed!( v => |v| BytesMut::from(v.to_le_bytes().as_ref()) )
            }
            PackedArray::SFixed64(v) => {
                write_packed!( v => |v| BytesMut::from(v.to_le_bytes().as_ref()) )
            }
            PackedArray::Bool(v) => {
                write_packed!( v => |v| BytesMut::from(if *v { [1u8].as_ref() } else { [0u8].as_ref() }))
            }
        };

        let mut output = data.len().into_unsigned_varint();
        output.extend_from_slice(data.as_ref());
        output
    }
}

fn return_incomplete(data: &mut &[u8], vt: u8, original: &[u8]) -> Value {
    *data = &[];
    Value::Incomplete(vt, Bytes::copy_from_slice(original))
}

fn try_read_8_bytes(data: &mut &[u8]) -> Option<[u8; 8]> {
    if data.len() < 8 {
        return None;
    }

    match (data[..8]).try_into() {
        Ok(v) => {
            *data = &data[8..];
            Some(v)
        }
        Err(_) => None,
    }
}

fn try_read_4_bytes(data: &mut &[u8]) -> Option<[u8; 4]> {
    if data.len() < 4 {
        return None;
    }

    match (data[..4]).try_into() {
        Ok(v) => {
            *data = &data[4..];
            Some(v)
        }
        Err(_) => None,
    }
}

fn read_string(data: &mut &[u8]) -> Option<String> {
    let original = *data;
    let len = usize::from_unsigned_varint(data)?;
    if len > data.len() {
        *data = original;
        return None;
    }
    let (str_data, remainder) = data.split_at(len);
    *data = remainder;
    Some(String::from_utf8_lossy(str_data).to_string())
}

fn read_bytes(data: &mut &[u8]) -> Option<Bytes> {
    let original = *data;
    let len = usize::from_unsigned_varint(data)?;
    if len > data.len() {
        *data = original;
        return None;
    }
    let (str_data, remainder) = data.split_at(len);
    *data = remainder;
    Some(Bytes::copy_from_slice(str_data))
}

impl MessageInfo {
    /// Decode a message.
    ///
    /// Will **panic** if the message defined by the `MessageRef` does not exist in this context.
    /// Such panic means the `MessageRef` came from a different context. The panic is not
    /// guaranteed, as a message with an equal `MessageRef` may exist in multiple contexts.
    pub fn decode(&self, mut data: &[u8], ctx: &Context) -> MessageValue {
        let mut msg = MessageValue {
            msg_ref: self.self_ref,
            fields: vec![],
            garbage: None,
        };

        loop {
            if data.is_empty() {
                break;
            }

            let tag = match u64::from_unsigned_varint(&mut data) {
                Some(tag) => tag,
                None => {
                    msg.garbage = Some(Bytes::copy_from_slice(data));
                    break;
                }
            };

            let number = tag >> 3;
            let wire_type = (tag & 0x07) as u8;

            let value = match self.get_field(number) {
                Some(field) => {
                    if field.multiplicity == Multiplicity::RepeatedPacked {
                        if wire_type == 2 {
                            Value::decode_packed(&mut data, wire_type, &field.field_type)
                        } else {
                            Value::decode_unknown(&mut data, wire_type)
                        }
                    } else if field.field_type.wire_type() == wire_type {
                        Value::decode(&mut data, wire_type, &field.field_type, ctx)
                    } else {
                        Value::decode_unknown(&mut data, wire_type)
                    }
                }
                _ => Value::decode_unknown(&mut data, wire_type),
            };

            msg.fields.push(FieldValue { number, value })
        }

        msg
    }
}

impl MessageValue {
    /// Encodes a message value into protobuf wire format.
    ///
    /// Will **panic** if the message defined by the `MessageRef` does not exist in this context.
    /// Such panic means the `MessageRef` came from a different context. The panic is not
    /// guaranteed, as a message with an equal `MessageRef` may exist in multiple contexts.
    pub fn encode(&self, ctx: &Context) -> bytes::BytesMut {
        self.fields
            .iter()
            .filter_map(|f| f.value.encode(ctx).map(|(w, b)| (f, w, b)))
            .flat_map(|(field, wire_type, bytes)| {
                let tag = wire_type as u64 + (field.number << 3);
                let mut field_data = tag.into_unsigned_varint();
                field_data.extend_from_slice(&bytes);
                field_data
            })
            .collect()
    }
}

impl UnknownValue {
    /// Encodes a message value into protobuf wire format.
    fn encode(&self) -> bytes::BytesMut {
        match self {
            UnknownValue::Varint(v) => v.into_unsigned_varint(),
            UnknownValue::Fixed64(v) => BytesMut::from(v.to_le_bytes().as_ref()),
            UnknownValue::VariableLength(b) => {
                let mut output = b.len().into_unsigned_varint();
                output.extend_from_slice(b);
                output
            }
            UnknownValue::Fixed32(v) => BytesMut::from(v.to_le_bytes().as_ref()),
            UnknownValue::Invalid(_, v) => BytesMut::from(v.as_ref()),
        }
    }
}

trait FromUnsignedVarint: Sized {
    fn from_unsigned_varint(data: &mut &[u8]) -> Option<Self>;
}

trait ToUnsignedVarint: Sized {
    fn into_unsigned_varint(self) -> BytesMut;
}

impl<T: Default + TryFrom<u64>> FromUnsignedVarint for T
where
    T::Error: Debug,
{
    fn from_unsigned_varint(data: &mut &[u8]) -> Option<Self> {
        let mut result = 0u64;
        let mut idx = 0;
        loop {
            if idx >= data.len() {
                return None;
            }

            let b = data[idx];
            let value = (b & 0x7f) as u64;
            result += value << (idx * 7);

            idx += 1;
            if b & 0x80 == 0 {
                break;
            }
        }

        let result = T::try_from(result).expect("Out of range");
        *data = &data[idx..];
        Some(result)
    }
}

impl<T: Default + TryInto<u64>> ToUnsignedVarint for T
where
    T::Error: Debug,
{
    fn into_unsigned_varint(self) -> BytesMut {
        let mut value: u64 = self.try_into().unwrap();
        let mut data: Vec<u8> = Vec::with_capacity(8);
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value > 0 {
                byte |= 0x80;
                data.push(byte);
            } else {
                data.push(byte);
                break BytesMut::from(data.as_slice());
            }
        }
    }
}

trait FromSignedVarint: Sized {
    fn from_signed_varint(data: &mut &[u8]) -> Option<Self>;
}

trait ToSignedVarint: Sized {
    fn into_signed_varint(self) -> BytesMut;
}

impl<T: Default + TryFrom<i64>> FromSignedVarint for T
where
    T::Error: Debug,
{
    fn from_signed_varint(data: &mut &[u8]) -> Option<Self> {
        u64::from_unsigned_varint(data).map(|u| {
            let signed: i64 = unsafe { std::mem::transmute(u) };
            signed.try_into().unwrap()
        })
    }
}

impl<T: Default + TryInto<i64>> ToSignedVarint for T
where
    T::Error: Debug,
{
    fn into_signed_varint(self) -> BytesMut {
        let v: u64 = unsafe { std::mem::transmute(self.try_into().unwrap()) };
        v.into_unsigned_varint()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_zigzag_encoding()
    // Source: https://developers.google.com/protocol-buffers/docs/encoding#signed-ints
    // Signed Original	Encoded As
    // 0	            0
    // -1	            1
    // 1	            2
    // -2	            3
    // 2147483647	    4294967294
    // -2147483648	    4294967295
    {
        let ctx = Context::parse(&[r#"
            syntax = "proto3";
            message Message {}
        "#])
        .unwrap();

        // Singular
        assert_eq!(Value::SInt32(0).encode(&ctx), Value::Int32(0).encode(&ctx));
        assert_eq!(Value::SInt32(-1).encode(&ctx), Value::Int32(1).encode(&ctx));
        assert_eq!(Value::SInt32(1).encode(&ctx), Value::Int32(2).encode(&ctx));
        assert_eq!(
            Value::SInt64(2147483647).encode(&ctx),
            Value::Int64(4294967294).encode(&ctx)
        );
        assert_eq!(
            Value::SInt64(-2147483648).encode(&ctx),
            Value::Int64(4294967295).encode(&ctx)
        );

        // Packed
        assert_eq!(
            Value::Packed(PackedArray::SInt32(vec![0, -1, 1, -2, 2])).encode(&ctx),
            Value::Packed(PackedArray::Int32(vec![0, 1, 2, 3, 4])).encode(&ctx),
        );
        assert_eq!(
            Value::Packed(PackedArray::SInt64(vec![0, 2147483647, -2147483648])).encode(&ctx),
            Value::Packed(PackedArray::Int64(vec![0, 4294967294, 4294967295])).encode(&ctx),
        );
    }
}
