# ::\<proto\>
### proto-file parser and arbitrary Protocol Buffer message decoder

[![crates.io](https://img.shields.io/crates/v/protofish.svg)](https://crates.io/crates/protofish)
[![Docs](https://docs.rs/protofish/badge.svg)](https://docs.rs/protofish)

Protofish is a decoder focused on decoding arbitrary protocol buffer messages
with error recovery. Its primary use case is decoding gRPC mesages in
[proxide](https://github.com/Rantanen/proxide) based on .proto-files supplied
by the user at runtime.

```rust
use protofish::{Context, Value, UnknownValue};
use bytes::Bytes;

let context = Context::parse(&[r#"
  syntax = "proto3";
  package Proto;

  message Request { string kind = 1; }
  message Response { int32 distance = 1; }
  service Fish {
    rpc Swim( Request ) returns ( Response );
  }
"#]).unwrap();

let service = context.get_service("Proto.Fish").unwrap();
let rpc = service.rpc_by_name("Swim").unwrap();

let input = rpc.input.message.decode(b"\x0a\x05Perch", &context);
assert_eq!(input.fields[0].number, 1);
assert_eq!(input.fields[0].value, Value::String(String::from("Perch")));

let output = rpc.output.message.decode(b"\x08\xa9\x46", &context);
assert_eq!(output.fields[0].number, 1);
assert_eq!(output.fields[0].value, Value::Int32(9001));
```

## Goals

- Protocol Buffers Version 3 support.
- Standalone proto-file parser that does not depend on `protoc`.
- Ability to decode partial and invalid Protocol Buffer messages.

## Explicitly not goals

- Extremely blazingly fast for MAXIMUM PERFORMANCE.
  - Speed is great, but correctness, error recovery and maintainability have
    higher priority.
  - Applies especially to parsing the proto-files.
- Protocol Buffers Version 2 support.
  - Not needed for decoding gRPC. Not opposed to including support, if doing
    so doesn't compromise maintainability.
- Code generation
  - There are few other crates that already do this.
- Validating the proto-files
  - Protofish is interested in parsing all valid proto-files. Rejecting invalid
    ones doesn't currently hold any value for being able to decode messages
    based on valid files. Validation isn't completely out of scope, but it's
    not a priority currently.

## Motivation

There are couple of other crate in the Rust ecosystem for handling Protocol
Buffer messages. Most of these crates focus on compile time code generation
for generating message types for runtime serialization. Most of these crates
also depend on `protoc` for the actual proto-file parsing.

The [quick-protobuf] project has a stand-alone proto-file parser: [pb-rs].
Unfortunately that parser is missing support for the full proto-file syntax (at
least `stream` requests and responses were unsupported in `rpc` definitions at
the time of writing this README).

Protofish uses [PEG][proto.pest] based on the published [Protocol
Buffers Version 3 Language Specification][proto-spec]. While that specification
is slightly inaccurate, writing the grammar based on the official EBNF syntax
provided an easy way to build a comprehensive parser.

A hand crafted Nom-based parser might be faster, but in most cases there is no
need for high performance when reading proto-files. Proxide for example does
this once at program startup.

[quick-protobuf]: https://github.com/tafia/quick-protobuf
[pb-rs]: https://crates.io/crates/pb-rs
[proto.pest]: src/proto.pest
[proto-spec]: https://developers.google.com/protocol-buffers/docs/reference/proto3-spec

## Missing features

### Handling `import` statements.. or not

Protofish _ignores_ `import` statements in the proto-files. Building a
comprehensive decoding context depends on processing all files that contain the
required types. This means whichever files the `import` statements refer to
need to be passed to protofish for parsing anyway. As a result there's little
need to parse the `import` statements early.

### Handling custom options

The `extends` syntax is not part of the published Protocol Buffers Version 3
Language Specification (which shows just how wrong that specification is). For
this reason the `extends` syntax used with custom options is not supported by
the parser.

Since protofish doesn't validate the options anyway, there will probably be
initial support for _skipping_ the `extends` definitions in the near future.
