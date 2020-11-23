//! Decoding context built from the proto-files.

use bytes::Bytes;
use snafu::{ResultExt, Snafu};
use std::collections::{BTreeMap, HashMap};

mod api;
mod builder;
mod parse;

/// Protofish error type.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
#[non_exhaustive]
pub enum ParseError
{
    /// Syntax error in the input files.
    #[snafu(display("Parsing error: {}", source))]
    SyntaxError
    {
        /// Source error.
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Duplicate type.
    #[snafu(display("Duplicate type: {}", name))]
    DuplicateType
    {
        /// Type.
        name: String,
    },

    /// Unknown type reference.
    #[snafu(display("Unknown type '{}' in '{}'", name, context))]
    TypeNotFound
    {
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
    InvalidTypeKind
    {
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
pub enum InsertError
{
    /// A type conflicts with an existing type.
    TypeExists
    {
        /// The previous type that conflicts with the new one.
        original: TypeRef,
    },
}

/// Type reference that references either message or enum type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeRef
{
    /// Message type reference.
    Message(MessageRef),

    /// Enum type reference.
    Enum(EnumRef),
}

/// Protobuf item type
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemType
{
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
#[derive(Default, Debug)]
pub struct Context
{
    packages: Vec<Package>,
    types: Vec<TypeInfo>,
    types_by_name: HashMap<String, usize>,
    services: Vec<Service>,
    services_by_name: HashMap<String, usize>,
}

#[derive(Default, Debug)]
struct Package
{
    name: Option<String>,
    types: Vec<usize>,
    services: Vec<usize>,
}

/// Message or enum type.
#[derive(Debug)]
pub enum TypeInfo
{
    /// Message.
    Message(MessageInfo),

    /// Enum.
    Enum(EnumInfo),
}

/// Message details
#[derive(Debug)]
pub struct MessageInfo
{
    /// Message name.
    pub name: String,

    /// Full message name, including package and parent type names.
    pub full_name: String,

    /// `MessageRef` that references this message.
    pub self_ref: MessageRef,

    /// Message fields.
    pub fields: BTreeMap<u64, MessageField>,

    /// `oneof` structures defined within the message.
    pub oneofs: Vec<Oneof>,

    /// References to the inner types defined within this message.
    pub inner_types: Vec<InnerType>,
}

/// Inner type reference.
#[derive(Debug)]
pub enum InnerType
{
    /// Inner `message`.
    Message(MessageRef),

    /// Inner `enum`.
    Enum(EnumRef),
}

/// Enum details
#[derive(Debug)]
pub struct EnumInfo
{
    /// Enum name.
    pub name: String,

    /// Full message name, including package and parent type names.
    pub full_name: String,

    /// `EnumRef` that references this enum.
    pub self_ref: EnumRef,

    /// Enum fields.
    pub fields: Vec<EnumField>,

    fields_by_value: HashMap<i64, usize>,
}

/// Message field details.
#[derive(Debug)]
pub struct MessageField
{
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
    pub oneof: Option<usize>,
}

/// Defines the multiplicity of the field values.
#[derive(Debug, PartialEq)]
pub enum Multiplicity
{
    /// Field is not repeated.
    Single,

    /// Field may be repeated.
    Repeated,

    /// Field is repeated by packing.
    RepeatedPacked,
}

/// Message `oneof` details.
#[derive(Debug)]
pub struct Oneof
{
    /// Name of the `oneof` structure.
    pub name: String,

    /// Field numbers of the fields contained in the `oneof`.
    pub fields: Vec<u64>,

    /// Options.
    pub options: Vec<ProtoOption>,
}

/// Enum field details.
#[derive(Debug, PartialEq, Clone)]
pub struct EnumField
{
    /// Enum field name.
    pub name: String,

    /// Enum field value.
    pub value: i64,

    /// Options.
    pub options: Vec<ProtoOption>,
}

/// Field value types.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueType
{
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InternalRef(usize);

/// A reference to a message. Can be resolved to `MessageInfo` through a `Context`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageRef(InternalRef);

/// A reference to an enum. Can be resolved to `EnumInfo` through a `Context`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumRef(InternalRef);

/// Service details
#[derive(Debug, PartialEq)]
pub struct Service
{
    /// Service name.
    pub name: String,

    /// Full service name, including the package name.
    pub full_name: String,

    /// List of `rpc` operations defined in the service.
    pub rpcs: Vec<Rpc>,

    /// Options.
    pub options: Vec<ProtoOption>,

    rpcs_by_name: HashMap<String, usize>,
}

/// Rpc operation
#[derive(Debug, PartialEq)]
pub struct Rpc
{
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
pub struct RpcArg
{
    /// References to the message type.
    pub message: MessageRef,

    /// True, if this is a stream.
    pub stream: bool,
}

/// A single option.
#[derive(Debug, PartialEq, Clone)]
pub struct ProtoOption
{
    /// Option name.
    pub name: String,

    /// Optionn value.
    pub value: Constant,
}

/// Constant value, used for options.
#[derive(Debug, PartialEq, Clone)]
pub enum Constant
{
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
