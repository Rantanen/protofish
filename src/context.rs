//! Decoding context built from the proto-files.

use snafu::{ResultExt, Snafu};
use std::collections::{BTreeMap, HashMap};

mod builder;
mod parse;

/// Protofish error type.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error
{
    /// Syntax error in the input files.
    #[snafu(display("Parsing error: {}", source))]
    ParseError
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

/// Result alias for the context errors.
pub type Result<S, E = Error> = std::result::Result<S, E>;

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
    pub repeated: bool,

    /// Field options.
    pub options: Vec<ProtoOption>,

    /// Index to the ´oneof` structure in the parent type if this field is part of a `oneof`.
    pub oneof: Option<usize>,
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct TypeRef(usize);

/// A reference to a message. Can be resolved to `MessageInfo` through a `Context`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MessageRef(TypeRef);

/// A reference to an enum. Can be resolved to `EnumInfo` through a `Context`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnumRef(TypeRef);

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
pub struct ProtoOption {}

impl Context
{
    /// Gets type info by name.
    pub fn get_type(&self, full_name: &str) -> Option<&TypeInfo>
    {
        self.types_by_name
            .get(full_name)
            .map(|idx| &self.types[*idx])
    }

    /// Gets a message type info by name.
    pub fn get_message(&self, full_name: &str) -> Option<&MessageInfo>
    {
        match self.get_type(full_name) {
            Some(TypeInfo::Message(m)) => Some(m),
            _ => None,
        }
    }

    fn resolve_type(&self, tr: TypeRef) -> Option<&TypeInfo>
    {
        self.types.get(tr.0)
    }

    /// Resolves a message reference.
    ///
    /// Will **panic** if the message defined by the `MessageRef` does not exist in this context.
    /// Such panic means the `MessageRef` came from a different context. The panic is not
    /// guaranteed, as a message with an equal `MessageRef` may exist in multiple contexts.
    pub fn resolve_message(&self, tr: MessageRef) -> &MessageInfo
    {
        match self.resolve_type(tr.0) {
            Some(TypeInfo::Message(msg)) => msg,
            _ => panic!("Message did not exist in this context"),
        }
    }

    /// Resolves a enum reference.
    ///
    /// Will **panic** if the enum defined by the `EnumRef` does not exist in this context.
    /// Such panic means the `EnumRef` came from a different context. The panic is not
    /// guaranteed, as an enum with an equal `EnumRef` may exist in multiple contexts.
    pub fn resolve_enum(&self, tr: EnumRef) -> &EnumInfo
    {
        match self.resolve_type(tr.0) {
            Some(TypeInfo::Enum(e)) => e,
            _ => panic!("Message did not exist in this context"),
        }
    }

    /// Gets a service by full name.
    pub fn get_service(&self, full_name: &str) -> Option<&Service>
    {
        self.services_by_name
            .get(full_name)
            .map(|idx| &self.services[*idx])
    }
}

impl TypeInfo
{
    pub(crate) fn full_name(&self) -> String
    {
        match self {
            TypeInfo::Message(v) => v.full_name.clone(),
            TypeInfo::Enum(v) => v.full_name.clone(),
        }
    }
}

impl EnumInfo
{
    /// Gets a field by value.
    ///
    /// If the field is aliased, an undefined field alias is returned.
    pub fn field_by_value(&self, value: i64) -> Option<&EnumField>
    {
        self.fields_by_value
            .get(&value)
            .map(|idx| &self.fields[*idx])
    }
}

impl Service
{
    /// Gets an `Rpc` info by operation name.
    pub fn rpc_by_name(&self, name: &str) -> Option<&Rpc>
    {
        self.rpcs_by_name.get(name).map(|idx| &self.rpcs[*idx])
    }
}

impl ValueType
{
    pub(crate) fn tag(&self) -> u8
    {
        match self {
            Self::Double => 1,
            Self::Float => 5,
            Self::Int32 => 0,
            Self::Int64 => 0,
            Self::UInt32 => 0,
            Self::UInt64 => 0,
            Self::SInt32 => 0,
            Self::SInt64 => 0,
            Self::Fixed32 => 5,
            Self::Fixed64 => 1,
            Self::SFixed32 => 5,
            Self::SFixed64 => 1,
            Self::Bool => 0,
            Self::String => 2,
            Self::Bytes => 2,
            Self::Message(..) => 2,
            Self::Enum(..) => 0,
        }
    }
}
