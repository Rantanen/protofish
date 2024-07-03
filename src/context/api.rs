use super::*;

impl Context {
    /// Create a new context.
    pub fn new() -> Self {
        Context {
            packages: Default::default(),
            types: Default::default(),
            types_by_name: Default::default(),
            services: Default::default(),
            services_by_name: Default::default(),
        }
    }

    /// Resolves a package reference.
    ///
    /// Will **panic** if the package defined by the `PackageRef` does not exist in this context.
    /// Such panic means the `PackageRef` came from a different context. The panic is not
    /// guaranteed, as a message with an equal `MessageRef` may exist in multiple contexts.
    pub fn resolve_package(&self, package_ref: PackageRef) -> &Package {
        &self.packages[package_ref.0 .0]
    }

    /// Gets type info by name.
    pub fn get_type(&self, full_name: &str) -> Option<&TypeInfo> {
        self.types_by_name
            .get(full_name)
            .map(|idx| &self.types[*idx])
    }

    /// Gets a message type info by name.
    pub fn get_message(&self, full_name: &str) -> Option<&MessageInfo> {
        match self.get_type(full_name) {
            Some(TypeInfo::Message(m)) => Some(m),
            _ => None,
        }
    }

    fn resolve_type(&self, tr: InternalRef) -> Option<&TypeInfo> {
        self.types.get(tr.0)
    }

    /// Resolves a message reference.
    ///
    /// Will **panic** if the message defined by the `MessageRef` does not exist in this context.
    /// Such panic means the `MessageRef` came from a different context. The panic is not
    /// guaranteed, as a message with an equal `MessageRef` may exist in multiple contexts.
    pub fn resolve_message(&self, tr: MessageRef) -> &MessageInfo {
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
    pub fn resolve_enum(&self, tr: EnumRef) -> &EnumInfo {
        match self.resolve_type(tr.0) {
            Some(TypeInfo::Enum(e)) => e,
            _ => panic!("Message did not exist in this context"),
        }
    }

    /// Gets a service by full name.
    pub fn get_service(&self, full_name: &str) -> Option<&Service> {
        self.services_by_name
            .get(full_name)
            .map(|idx| &self.services[*idx])
    }
}

impl TypeInfo {
    /// Get the full name of the type.
    pub fn name(&self) -> &str {
        match self {
            TypeInfo::Message(m) => &m.name,
            TypeInfo::Enum(e) => &e.name,
        }
    }

    /// Get the full name of the type.
    pub fn full_name(&self) -> &str {
        match self {
            TypeInfo::Message(m) => &m.full_name,
            TypeInfo::Enum(e) => &e.full_name,
        }
    }

    /// Get the parent information for the type.
    pub fn parent(&self) -> TypeParent {
        match self {
            TypeInfo::Message(m) => m.parent,
            TypeInfo::Enum(e) => e.parent,
        }
    }
}

impl MessageInfo {
    /// Iterates all message fields.
    pub fn iter_fields(&self) -> impl Iterator<Item = &MessageField> {
        self.fields.values()
    }

    /// Get a field by its number.
    pub fn get_field(&self, number: u64) -> Option<&MessageField> {
        self.fields.get(&number)
    }

    /// Get a field by its name.
    pub fn get_field_by_name(&self, name: &str) -> Option<&MessageField> {
        self.fields_by_name
            .get(name)
            .and_then(|id| self.get_field(*id))
    }

    /// Gets a oneof by a oneof reference.
    pub fn get_oneof(&self, oneof: OneofRef) -> Option<&Oneof> {
        self.oneofs.iter().find(|oo| oo.self_ref == oneof)
    }
}

impl EnumInfo {
    /// Gets a field by value.
    ///
    /// If the field is aliased, an undefined field alias is returned.
    pub fn get_field_by_value(&self, value: i64) -> Option<&EnumField> {
        self.fields_by_value.get(&value)
    }
}

impl Service {
    /// Gets an `Rpc` info by operation name.
    pub fn rpc_by_name(&self, name: &str) -> Option<&Rpc> {
        self.rpcs_by_name.get(name).map(|idx| &self.rpcs[*idx])
    }
}

impl ValueType {
    pub(crate) fn wire_type(&self) -> u8 {
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
