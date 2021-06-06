//!
//! Protofish is a decoder focused on decoding arbitrary protocol buffer messages
//! with error recovery. Its primary use case is decoding gRPC mesages in
//! [proxide](https://github.com/Rantanen/proxide) based on .proto-files supplied
//! by the user at runtime.
//!
//!
//! ```
//! use protofish::prelude::*;
//! use protofish::decode::UnknownValue;
//! use bytes::Bytes;
//!
//! let context = Context::parse(&[r#"
//!   syntax = "proto3";
//!   package Proto;
//!
//!   message Request { string kind = 1; }
//!   message Response { int32 distance = 1; }
//!   service Fish {
//!     rpc Swim( Request ) returns ( Response );
//!   }
//! "#]).unwrap();
//!
//! let service = context.get_service("Proto.Fish").unwrap();
//! let rpc = service.rpc_by_name("Swim").unwrap();
//!
//! let input = context.decode(rpc.input.message, b"\x0a\x05Perch");
//! assert_eq!(input.fields[0].number, 1);
//! assert_eq!(input.fields[0].value, Value::String(String::from("Perch")));
//!
//! let output = context.decode(rpc.output.message, b"\x08\xa9\x46");
//! assert_eq!(output.fields[0].number, 1);
//! assert_eq!(output.fields[0].value, Value::Int32(9001));
//!
//! let bytes = b"\x12\x07Unknown\x0a\x0fAtlantic ";
//! let request = context.get_message("Proto.Request").unwrap();
//! let value = request.decode(bytes, &context);
//! assert_eq!(value.fields[0].number, 2);
//! assert_eq!(
//!     value.fields[0].value,
//!     Value::Unknown(UnknownValue::VariableLength(Bytes::from_static(b"Unknown"))));
//! assert_eq!(
//!     value.fields[1].value,
//!     Value::Incomplete(2, Bytes::from_static(b"\x0fAtlantic ")));
//!
//! let encoded = value.encode(&context);
//! assert_eq!(encoded, &bytes[..]);
//! ```
#![warn(missing_docs)]
#![allow(clippy::match_bool)]

pub mod context;
pub mod decode;
pub mod prelude;
