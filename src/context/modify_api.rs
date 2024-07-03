use super::*;

impl Context {
    /// Insert a new message definition to the context.
    pub fn insert_message(&mut self, ty: MessageInfo) -> Result<MessageRef, InsertError> {
        self.insert_type(TypeInfo::Message(ty)).map(MessageRef)
    }

    /// Insert a new enum definition to the context.
    pub fn insert_enum(&mut self, ty: EnumInfo) -> Result<EnumRef, InsertError> {
        self.insert_type(TypeInfo::Enum(ty)).map(EnumRef)
    }

    /// Insert a new package to the context.
    ///
    /// Returns an error if the package with the same name already exists.
    pub fn insert_package(&mut self, mut pkg: Package) -> Result<PackageRef, PackageRef> {
        let pkg_ref = PackageRef(InternalRef(self.packages.len()));
        for existing in &self.packages {
            if existing.name == pkg.name {
                return Err(existing.self_ref);
            }
        }

        pkg.self_ref = pkg_ref;
        self.packages.push(pkg);
        Ok(pkg_ref)
    }

    fn insert_type(&mut self, mut ty: TypeInfo) -> Result<InternalRef, InsertError> {
        use std::collections::hash_map::Entry;

        // First validate the operation. We'll want to ensure the operation succeeds before we make
        // _any_ changes to the context to avoid making partial changes in case of a failure.

        let internal_ref = InternalRef(self.types.len());
        let parent = ty.parent();

        let full_name = match parent {
            TypeParent::Package(p) => {
                let package = &self.packages[p.0 .0];
                match &package.name {
                    Some(package_name) => format!("{}.{}", package_name, ty.name()),
                    None => ty.name().to_string(),
                }
            }
            TypeParent::Message(m) => {
                let msg = &self.types[m.0 .0];
                format!("{}.{}", msg.full_name(), ty.name())
            }
        };

        match &mut ty {
            TypeInfo::Message(m) => m.full_name = full_name.clone(),
            TypeInfo::Enum(e) => e.full_name = full_name.clone(),
        }

        let vacant = match self.types_by_name.entry(full_name) {
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

        // From here on, we're modifying the context.
        // All validations should be done now.

        // Add to the parent collection. Either to the package types or message inner types.
        match parent {
            TypeParent::Package(p) => {
                let package = &mut self.packages[p.0 .0];
                package.types.push(type_ref);
            }
            TypeParent::Message(m) => {
                let ty_info = &mut self.types[m.0 .0];
                match ty_info {
                    TypeInfo::Message(msg) => msg.inner_types.push(type_ref),
                    _ => panic!("Inner type for a non-Message"),
                }
            }
        };

        vacant.insert(internal_ref.0);
        self.types.push(ty);

        Ok(internal_ref)
    }
}

impl Package {
    /// Create a new package.
    pub fn new(name: Option<String>) -> Self {
        Self {
            name,
            self_ref: PackageRef(InternalRef(0)),
            types: vec![],
            services: vec![],
        }
    }
}

impl MessageInfo {
    /// Create a new message info.
    ///
    /// Before inserting the message info into a [`Context`] certain fields such as `self_ref` or
    /// `full_name` are not valid.
    pub fn new(name: String, parent: TypeParent) -> Self {
        MessageInfo {
            name,
            parent,

            full_name: String::new(),
            self_ref: MessageRef(InternalRef(0)),
            oneofs: vec![],
            inner_types: vec![],

            fields: BTreeMap::new(),
            fields_by_name: BTreeMap::new(),
        }
    }

    /// Add a field to the type.
    pub fn add_field(&mut self, field: MessageField) -> Result<(), MemberInsertError> {
        use std::collections::btree_map::Entry;

        let num = field.number;
        let num_entry = self.fields.entry(num);
        let name_entry = self.fields_by_name.entry(field.name.to_string());

        let (vacant_num, vacant_name) = match (num_entry, name_entry) {
            (Entry::Occupied(..), _) => return Err(MemberInsertError::NumberConflict),
            (_, Entry::Occupied(..)) => return Err(MemberInsertError::NameConflict),
            (Entry::Vacant(num), Entry::Vacant(name)) => (num, name),
        };

        if let Some(oneof_ref) = field.oneof {
            let oneof = self
                .oneofs
                .get_mut(oneof_ref.0 .0)
                .ok_or(MemberInsertError::MissingOneof)?;
            oneof.fields.push(num);
        }

        vacant_num.insert(field);
        vacant_name.insert(num);

        Ok(())
    }

    /// Add a oneof record to the message.
    pub fn add_oneof(&mut self, mut oneof: Oneof) -> Result<OneofRef, OneofInsertError> {
        let oneof_ref = OneofRef(InternalRef(self.oneofs.len()));
        for o in &self.oneofs {
            if o.name == oneof.name {
                return Err(OneofInsertError::NameConflict);
            }
        }

        // Ensure none of the existing fields are part of oneofs.
        for f in &oneof.fields {
            self.fields
                .get(f)
                .ok_or(OneofInsertError::FieldNotFound { field: *f })?;
        }

        // From here on we're making changes to self.
        // No error should be raised anymore to avoid partial changes.

        for f in &mut oneof.fields {
            let f = self.fields.get_mut(f).expect("Field disappeared");
            f.oneof = Some(oneof_ref);
        }

        oneof.self_ref = oneof_ref;
        self.oneofs.push(oneof);

        Ok(oneof_ref)
    }
}

impl MessageField {
    /// Create a new message field.
    pub fn new(name: String, number: u64, field_type: ValueType) -> Self {
        Self {
            name,
            number,
            field_type,
            multiplicity: Multiplicity::Single,
            options: vec![],
            oneof: None,
        }
    }
}

impl Oneof {
    /// Create a new Oneof definition.
    pub fn new(name: String) -> Self {
        Self {
            name,
            self_ref: OneofRef(InternalRef(0)),
            fields: vec![],
            options: vec![],
        }
    }
}

impl EnumInfo {
    /// Create a new enum info.
    pub fn new(name: String, parent: TypeParent) -> Self {
        Self {
            name,
            parent,
            full_name: String::new(),
            self_ref: EnumRef(InternalRef(0)),
            fields_by_value: BTreeMap::new(),
            fields_by_name: BTreeMap::new(),
        }
    }

    /// Add a field to the enum definition.
    pub fn add_field(&mut self, field: EnumField) -> Result<(), MemberInsertError> {
        use std::collections::btree_map::Entry;

        let value = field.value;
        let value_entry = self.fields_by_value.entry(value);
        let name_entry = self.fields_by_name.entry(field.name.to_string());

        let (vacant_value, vacant_name) = match (value_entry, name_entry) {
            (Entry::Occupied(..), _) => return Err(MemberInsertError::NumberConflict),
            (_, Entry::Occupied(..)) => return Err(MemberInsertError::NameConflict),
            (Entry::Vacant(value), Entry::Vacant(name)) => (value, name),
        };

        vacant_value.insert(field);
        vacant_name.insert(value);

        Ok(())
    }
}

impl EnumField {
    /// Create a new enum field.
    pub fn new(name: String, value: i64) -> Self {
        Self {
            name,
            value,
            options: vec![],
        }
    }
}
