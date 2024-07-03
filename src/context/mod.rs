//! Decoding context built from the proto-files.

use bytes::Bytes;
use snafu::{ResultExt, Snafu};
use std::collections::{BTreeMap, HashMap};

mod api;
mod builder;
mod modify_api;
mod parse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InternalRef(usize);

/// A reference to a message. Can be resolved to `MessageInfo` through a `Context`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageRef(InternalRef);

/// A reference to an enum. Can be resolved to `EnumInfo` through a `Context`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumRef(InternalRef);

/// A reference to a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageRef(InternalRef);

/// A reference to a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceRef(InternalRef);

/// A reference to a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OneofRef(InternalRef);

/// Protofish error type.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
#[non_exhaustive]
pub enum ParseError {
    /// Syntax error in the input files.
    #[snafu(display("Parsing error: {}", source))]
    SyntaxError {
        /// Source error.
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Duplicate type.
    #[snafu(display("Duplicate type: {}", name))]
    DuplicateType {
        /// Type.
        name: String,
    },

    /// Unknown type reference.
    #[snafu(display("Unknown type '{}' in '{}'", name, context))]
    TypeNotFound {
        /// Type name.
        name: String,
        /// Type that referred to the unknown type.
        context: String,
    },

    /// Wrong kind of type used in a specific context.
    #[snafu(display(
        "Invalid type '{}' ({:?}) for {}, expected {:?}",
        type_name,
        actual,
        context,
        expected
    ))]
    InvalidTypeKind {
        /// Type that is of the wrong kind.
        type_name: String,

        /// The context where the type was used.
        context: &'static str,

        /// Expected item type.
        expected: ItemType,

        /// Actual item type.
        actual: ItemType,
    },
}

/// Error modifying the context.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum InsertError {
    /// A type conflicts with an existing type.
    TypeExists {
        /// The previous type that conflicts with the new one.
        original: TypeRef,
    },
}

/// Error modifying a type.
#[derive(Debug)]
#[non_exhaustive]
pub enum MemberInsertError {
    /// A field with the same number already exists.
    NumberConflict,

    /// A field with the same name already exists.
    NameConflict,

    /// A field refers to a oneof that does not exist.
    MissingOneof,
}

/// Error modifying a type.
#[derive(Debug)]
#[non_exhaustive]
pub enum OneofInsertError {
    /// A oneof with the same name already exists.
    NameConflict,

    /// The oneof refers to a field that doesn't exist.
    FieldNotFound {
        /// Field number the Oneof referenced.
        field: u64,
    },
}

/// Type reference that references either message or enum type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeRef {
    /// Message type reference.
    Message(MessageRef),

    /// Enum type reference.
    Enum(EnumRef),
}

/// Protobuf item type
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemType {
    /// `message` item
    Message,

    /// `enum` item
    Enum,

    /// `service` item
    Service,
}

/// Protofish decoding context.
///
/// Contains type information parsed from the files. Required for decoding
/// incoming Protobuf messages.
#[derive(Default, Debug, PartialEq)]
pub struct Context {
    packages: Vec<Package>,
    types: Vec<TypeInfo>,
    types_by_name: HashMap<String, usize>,
    services: Vec<Service>,
    services_by_name: HashMap<String, usize>,
}

/// Package details.
#[derive(Debug, PartialEq)]
pub struct Package {
    /// Package name. None for an anonymous package.
    name: Option<String>,

    /// Package self reference.
    self_ref: PackageRef,

    /// Top level types.
    types: Vec<TypeRef>,

    /// Services.
    services: Vec<usize>,
}

/// Message or enum type.
#[derive(Debug, PartialEq)]
pub enum TypeInfo {
    /// Message.
    Message(MessageInfo),

    /// Enum.
    Enum(EnumInfo),
}

/// Message details
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct MessageInfo {
    /// Message name.
    pub name: String,

    /// Full message name, including package and parent type names.
    pub full_name: String,

    /// Parent
    pub parent: TypeParent,

    /// `MessageRef` that references this message.
    pub self_ref: MessageRef,

    /// `oneof` structures defined within the message.
    pub oneofs: Vec<Oneof>,

    /// References to the inner types defined within this message.
    pub inner_types: Vec<TypeRef>,

    // Using BTreeMap here to ensure ordering.
    fields: BTreeMap<u64, MessageField>,
    fields_by_name: BTreeMap<String, u64>,
}

/// Reference to a type parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeParent {
    /// Reference to a package for top-level types.
    Package(PackageRef),

    /// Reference to a message for inner types.
    Message(MessageRef),
}

/// Enum details
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct EnumInfo {
    /// Enum name.
    pub name: String,

    /// Full message name, including package and parent type names.
    pub full_name: String,

    /// Parent
    pub parent: TypeParent,

    /// `EnumRef` that references this enum.
    pub self_ref: EnumRef,

    fields_by_value: BTreeMap<i64, EnumField>,
    fields_by_name: BTreeMap<String, i64>,
}

/// Message field details.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct MessageField {
    /// Field name.
    pub name: String,

    /// Field number.
    pub number: u64,

    /// Field type
    pub field_type: ValueType,

    /// True, if this field is a repeated field.
    pub multiplicity: Multiplicity,

    /// Field options.
    pub options: Vec<ProtoOption>,

    /// Index to the ´oneof` structure in the parent type if this field is part of a `oneof`.
    pub oneof: Option<OneofRef>,
}

/// Defines the multiplicity of the field values.
#[derive(Debug, PartialEq, Clone)]
pub enum Multiplicity {
    /// Field is not repeated.
    Single,

    /// Field may be repeated.
    Repeated,

    /// Field is repeated by packing.
    RepeatedPacked,

    /// Field is optional.
    Optional,
}

/// Message `oneof` details.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct Oneof {
    /// Name of the `oneof` structure.
    pub name: String,

    /// Self reference of the `Oneof` in the owning type.
    pub self_ref: OneofRef,

    /// Field numbers of the fields contained in the `oneof`.
    pub fields: Vec<u64>,

    /// Options.
    pub options: Vec<ProtoOption>,
}

/// Enum field details.
#[derive(Debug, PartialEq, Clone)]
#[non_exhaustive]
pub struct EnumField {
    /// Enum field name.
    pub name: String,

    /// Enum field value.
    pub value: i64,

    /// Options.
    pub options: Vec<ProtoOption>,
}

/// Field value types.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueType {
    /// `double`
    Double,

    /// `float`
    Float,

    /// `int32`
    Int32,

    /// `int64`
    Int64,

    /// `uint32`
    UInt32,

    /// `uint64`
    UInt64,

    /// `sint32`
    SInt32,

    /// `sint64`
    SInt64,

    /// `fixed32`
    Fixed32,

    /// `fixed64`
    Fixed64,

    /// `sfixed32`
    SFixed32,

    /// `sfixed64`
    SFixed64,

    /// `bool`
    Bool,

    /// `string`
    String,

    /// `bytes`
    Bytes,

    /// A message type.
    Message(MessageRef),

    /// An enum type.
    Enum(EnumRef),
}

/// Service details
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct Service {
    /// Service name.
    pub name: String,

    /// Full service name, including the package name.
    pub full_name: String,

    /// Service self reference.
    pub self_ref: ServiceRef,

    /// Package that contains the service.
    pub parent: PackageRef,

    /// List of `rpc` operations defined in the service.
    pub rpcs: Vec<Rpc>,

    /// Options.
    pub options: Vec<ProtoOption>,

    rpcs_by_name: HashMap<String, usize>,
}

/// Rpc operation
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct Rpc {
    /// Operation name.
    pub name: String,

    /// Input details.
    pub input: RpcArg,

    /// Output details.
    pub output: RpcArg,

    /// Options.
    pub options: Vec<ProtoOption>,
}

/// Rpc operation input or output details.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct RpcArg {
    /// References to the message type.
    pub message: MessageRef,

    /// True, if this is a stream.
    pub stream: bool,
}

/// A single option.
#[derive(Debug, PartialEq, Clone)]
pub struct ProtoOption {
    /// Option name.
    pub name: String,

    /// Optionn value.
    pub value: Constant,
}

/// Constant value, used for options.
#[derive(Debug, PartialEq, Clone)]
pub enum Constant {
    /// An ident `foo.bar.baz`.
    Ident(String),

    /// An integer constant.
    Integer(i64),

    /// A floating point constant.
    Float(f64),

    /// A string constant.
    ///
    /// The string isn't guaranteed to be well formed UTF-8 so it's stored as
    /// Bytes here.
    String(Bytes),

    /// A boolean constant.
    Bool(bool),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic_package() {
        let ctx = Context::parse(&[r#"
            syntax = "proto3";
            message Message {}
        "#])
        .unwrap();

        let m = ctx.get_message("Message").unwrap();
        assert_eq!(m.parent, TypeParent::Package(PackageRef(InternalRef(0))));
    }

    #[test]
    fn basic_multiple_package() {
        let ctx = Context::parse(&[
            r#"
                syntax = "proto3";
                package First;
                message Message {}
            "#,
            r#"
                syntax = "proto3";
                package Second;
                message Message {}
            "#,
        ])
        .unwrap();

        let m = ctx.get_message("First.Message").unwrap();
        let pkg_ref = match m.parent {
            TypeParent::Package(p) => p,
            _ => panic!("Not a package reference: {:?}", m.parent),
        };
        let pkg = ctx.resolve_package(pkg_ref);
        assert_eq!(m.parent, TypeParent::Package(PackageRef(InternalRef(0))));
        assert_eq!(pkg.name.as_deref(), Some("First"));
        assert_eq!(pkg.types.len(), 1);

        let m = ctx.get_message("Second.Message").unwrap();
        let pkg_ref = match m.parent {
            TypeParent::Package(p) => p,
            _ => panic!("Not a package reference: {:?}", m.parent),
        };
        let pkg = ctx.resolve_package(pkg_ref);
        assert_eq!(m.parent, TypeParent::Package(PackageRef(InternalRef(1))));
        assert_eq!(pkg.name.as_deref(), Some("Second"));
        assert_eq!(pkg.types.len(), 1);
    }
}
