use super::*;

impl Context
{
    /// Create a new context.
    pub fn new() -> Self
    {
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
        &self.packages[package_ref.0.0]
    }

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

    fn resolve_type(&self, tr: InternalRef) -> Option<&TypeInfo>
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

    /// Insert a new message definition to the context.
    pub fn insert_message(&mut self, ty: MessageInfo) -> Result<MessageRef, InsertError>
    {
        self.insert_type(TypeInfo::Message(ty)).map(MessageRef)
    }

    /// Insert a new enum definition to the context.
    pub fn insert_enum(&mut self, ty: EnumInfo) -> Result<EnumRef, InsertError>
    {
        self.insert_type(TypeInfo::Enum(ty)).map(EnumRef)
    }

    fn insert_type(&mut self, mut ty: TypeInfo) -> Result<InternalRef, InsertError>
    {
        use std::collections::hash_map::Entry;

        // First validate the operation. We'll want to ensure the operation succeeds before we make
        // _any_ changes to the context to avoid making partial changes in case of a failure.
        let internal_ref = InternalRef(self.types.len());
        let full_name = ty.full_name();

        let mut name_split = full_name.rsplitn(1, ".");
        let _type_name = name_split
            .next()
            .expect("Name should have at least one segment");
        let package_name = name_split.next();

        let package_idx = self.find_package_index(package_name);

        let vacant = match self.types_by_name.entry(full_name.clone()) {
            Entry::Occupied(occupied) => {
                let original_ref = InternalRef(*occupied.get());
                let original = match self.types[original_ref.0] {
                    TypeInfo::Message(..) => TypeRef::Message(MessageRef(original_ref)),
                    TypeInfo::Enum(..) => TypeRef::Enum(EnumRef(original_ref)),
                };
                return Err(InsertError::TypeExists { original });
            }
            Entry::Vacant(vacant) => vacant,
        };

        // From here on, we're modifying the context.
        // All validations should be done now.

        let package_idx = match package_idx {
            Some(idx) => idx,
            None => {
                let idx = self.packages.len();
                self.packages.push(Package {
                    name: package_name.map(str::to_string),
                    types: vec![],
                    services: vec![],
                });
                idx
            }
        };
        let package = &mut self.packages[package_idx];

        let type_ref = match &mut ty {
            TypeInfo::Message(m) => {
                m.self_ref = MessageRef(internal_ref);
                TypeRef::Message(m.self_ref)
            }
            TypeInfo::Enum(e) => {
                e.self_ref = EnumRef(internal_ref);
                TypeRef::Enum(e.self_ref)
            }
        };

        vacant.insert(internal_ref.0);
        self.types.push(ty);
        package.types.push(type_ref);

        Ok(internal_ref)
    }

    fn find_package_index(&self, name: Option<&str>) -> Option<usize>
    {
        self.packages
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                if p.name.as_deref() == name {
                    Some(i)
                } else {
                    None
                }
            })
            .next()
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
    pub(crate) fn wire_type(&self) -> u8
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
