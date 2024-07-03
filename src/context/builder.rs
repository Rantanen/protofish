use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;

#[derive(Default)]
pub(crate) struct ContextBuilder {
    pub(crate) packages: Vec<PackageBuilder>,
}

#[derive(Default, Debug, PartialEq)]
pub(crate) struct PackageBuilder {
    pub(crate) path: PathBuf,
    pub(crate) name: Option<String>,
    pub(crate) imported_types: Vec<String>,
    pub(crate) types: Vec<ProtobufItemBuilder>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum ProtobufItemBuilder {
    Type(ProtobufTypeBuilder),
    Service(ServiceBuilder),
}

#[derive(Debug, PartialEq)]
pub(crate) enum ProtobufTypeBuilder {
    Message(MessageBuilder),
    Enum(EnumBuilder),
}

#[derive(Default, Debug, PartialEq, Clone)]
pub(crate) struct MessageBuilder {
    pub(crate) name: String,
    pub(crate) fields: Vec<FieldBuilder>,
    pub(crate) oneofs: Vec<OneofBuilder>,
    pub(crate) inner_types: Vec<InnerTypeBuilder>,
    pub(crate) options: Vec<ProtoOption>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct PseudoMapBuilder {
    pub(crate) key_type: FieldTypeBuilder,
    pub(crate) value_type: FieldTypeBuilder,
    pub(crate) field_name: String,
    pub(crate) number: u64,
    pub(crate) options: Vec<ProtoOption>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum InnerTypeBuilder {
    Message(MessageBuilder),
    Enum(EnumBuilder),
}

#[derive(Default, Debug, PartialEq, Clone)]
pub(crate) struct EnumBuilder {
    pub(crate) name: String,
    pub(crate) fields: Vec<EnumField>,
    pub(crate) options: Vec<ProtoOption>,
}

#[derive(Default, Debug, PartialEq)]
pub(crate) struct ServiceBuilder {
    pub(crate) name: String,
    pub(crate) rpcs: Vec<RpcBuilder>,
    pub(crate) options: Vec<ProtoOption>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct FieldBuilder {
    pub(crate) multiplicity: Multiplicity,
    pub(crate) field_type: FieldTypeBuilder,
    pub(crate) name: String,
    pub(crate) number: u64,
    pub(crate) options: Vec<ProtoOption>,
}

#[derive(Default, Debug, PartialEq, Clone)]
pub(crate) struct OneofBuilder {
    pub(crate) name: String,
    pub(crate) fields: Vec<FieldBuilder>,
    pub(crate) options: Vec<ProtoOption>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum FieldTypeBuilder {
    Builtin(ValueType),
    Unknown(String),
}

#[derive(Default, Debug, PartialEq)]
pub(crate) struct RpcBuilder {
    pub(crate) name: String,
    pub(crate) input: RpcArgBuilder,
    pub(crate) output: RpcArgBuilder,
    pub(crate) options: Vec<ProtoOption>,
}

#[derive(Default, Debug, PartialEq)]
pub(crate) struct RpcArgBuilder {
    pub(crate) stream: bool,
    pub(crate) message: String,
}

impl ContextBuilder {
    pub fn build(mut self) -> Result<Context, ParseError> {
        let mut cache = BuildCache::default();
        for (i, p) in self.packages.iter().enumerate() {
            p.populate(&mut cache, &mut vec![i])?;
        }

        // Iterate the types through the cache, since the cache has enough
        // details to find the original type, the types don't have details
        // to find the cache data without re-building the full path.
        let mut types = vec![];
        for cache_data in &cache.types {
            match cache_data.item_type {
                ItemType::Message | ItemType::Enum => {
                    let ty = self.take_type(&cache_data.idx_path);
                    let mut t = ty.build(cache_data, &cache)?;
                    match &mut t {
                        TypeInfo::Message(m) => assert_eq!(m.self_ref.0 .0, types.len()),
                        TypeInfo::Enum(e) => assert_eq!(e.self_ref.0 .0, types.len()),
                    }
                    types.push(t);
                }
                ItemType::Service => unreachable!("Service in type cache"),
            }
        }

        let services: Vec<_> = cache
            .services
            .iter()
            .map(|s| self.take_service(&s.idx_path).build(s, &cache))
            .collect::<Result<_, _>>()?;

        let types_by_name = types
            .iter()
            .enumerate()
            .map(|(idx, t)| (t.full_name().to_string(), idx))
            .collect();
        let services_by_name = services
            .iter()
            .enumerate()
            .map(|(idx, t)| (t.full_name.clone(), idx))
            .collect();

        let mut packages: Vec<Package> = self
            .packages
            .into_iter()
            .enumerate()
            .map(|(idx, p)| Package {
                name: p.name,
                self_ref: PackageRef(InternalRef(idx)),
                types: Vec::new(),
                services: Vec::new(),
            })
            .collect();

        for s in &services {
            let p = &mut packages[s.parent.0 .0];
            p.services.push(s.self_ref.0 .0);
        }

        for t in &types {
            let (raw_self_ref, parent) = match t {
                TypeInfo::Message(m) => (TypeRef::Message(m.self_ref), &m.parent),
                TypeInfo::Enum(e) => (TypeRef::Enum(e.self_ref), &e.parent),
            };
            match parent {
                TypeParent::Package(p_ref) => {
                    let p = &mut packages[p_ref.0 .0];
                    p.types.push(raw_self_ref);
                }
                TypeParent::Message(_) => {
                    // Messages handle their inner types on their own.
                }
            }
        }

        Ok(Context {
            packages,
            types,
            types_by_name,
            services,
            services_by_name,
        })
    }

    fn take_type(&mut self, idx: &[usize]) -> ProtobufTypeBuilder {
        self.packages[idx[0]].take_type(&idx[1..])
    }

    fn take_service(&mut self, idx: &[usize]) -> ServiceBuilder {
        self.packages[idx[0]].take_service(&idx[1..])
    }
}

impl PackageBuilder {
    fn populate(&self, cache: &mut BuildCache, idx: &mut Vec<usize>) -> Result<(), ParseError> {
        let mut path = match &self.name {
            Some(name) => name.split('.').collect(),
            None => vec![],
        };

        idx.push(0);
        for (i, t) in self.types.iter().enumerate() {
            *idx.last_mut().unwrap() = i;

            match t {
                ProtobufItemBuilder::Type(ProtobufTypeBuilder::Message(m)) => {
                    m.populate(cache, &mut path, idx)?
                }
                ProtobufItemBuilder::Type(ProtobufTypeBuilder::Enum(e)) => {
                    e.populate(cache, &mut path, idx)?
                }
                ProtobufItemBuilder::Service(m) => m.populate(cache, &mut path, idx)?,
            }
        }
        idx.pop();

        Ok(())
    }

    fn take_type(&mut self, idx: &[usize]) -> ProtobufTypeBuilder {
        match &mut self.types[idx[0]] {
            ProtobufItemBuilder::Type(t) => match t {
                ProtobufTypeBuilder::Message(m) => m.take_type(&idx[1..]),
                ProtobufTypeBuilder::Enum(e) => e.take_type(&idx[1..]),
            },

            // Panic here means something went wrong in populating the cache
            ProtobufItemBuilder::Service(..) => {
                panic!("Trying to take a service as a type");
            }
        }
    }

    fn take_service(&mut self, idx: &[usize]) -> ServiceBuilder {
        match &mut self.types[idx[0]] {
            ProtobufItemBuilder::Service(e) => std::mem::take(e),

            // Panic here means something went wrong in populating the cache
            _ => panic!("Trying to take a non-service as a service"),
        }
    }
}

impl ProtobufTypeBuilder {
    fn build(self, self_data: &CacheData, cache: &BuildCache) -> Result<TypeInfo, ParseError> {
        Ok(match self {
            ProtobufTypeBuilder::Message(m) => TypeInfo::Message(m.build(self_data, cache)?),
            ProtobufTypeBuilder::Enum(e) => TypeInfo::Enum(e.build(self_data, cache)?),
        })
    }
}

impl MessageBuilder {
    /// Lists types found in this message builder recursively into the build cache.
    ///
    /// On error the `path` and `idx` will be left in an undefined state.
    fn populate<'a>(
        &'a self,
        cache: &mut BuildCache,
        path: &mut Vec<&'a str>,
        idx: &mut Vec<usize>,
    ) -> Result<(), ParseError> {
        path.push(&self.name);
        let full_name = path.join(".");
        let cache_idx = cache.types.len();
        if cache
            .items
            .insert(full_name.clone(), (ItemType::Message, cache_idx))
            .is_some()
            || cache
                .items_by_idx
                .insert(idx.clone(), (ItemType::Message, cache_idx))
                .is_some()
        {
            return Err(ParseError::DuplicateType {
                name: path.join("."),
            });
        }

        // Make sure to push the current type into the cache before processing the possible
        // embedded types.
        cache.types.push(CacheData {
            item_type: ItemType::Message,
            full_name,
            idx_path: idx.clone(),
            final_idx: cache_idx,
        });

        idx.push(0);
        for (i, t) in self.inner_types.iter().enumerate() {
            *idx.last_mut().unwrap() = i;
            t.populate(cache, path, idx)?;
        }

        idx.pop();
        path.pop();

        Ok(())
    }

    fn take_type(&mut self, idx: &[usize]) -> ProtobufTypeBuilder {
        if idx.is_empty() {
            ProtobufTypeBuilder::Message(MessageBuilder {
                name: self.name.clone(),
                fields: std::mem::take(&mut self.fields),
                oneofs: std::mem::take(&mut self.oneofs),
                options: std::mem::take(&mut self.options),
                inner_types: self
                    .inner_types
                    .iter()
                    .map(InnerTypeBuilder::clone_name)
                    .collect(),
            })
        } else {
            self.inner_types[idx[0]].take_type(&idx[1..])
        }
    }

    fn build(self, self_data: &CacheData, cache: &BuildCache) -> Result<MessageInfo, ParseError> {
        let inner_types: Vec<_> = self
            .inner_types
            .iter()
            .map(|inner| match inner {
                InnerTypeBuilder::Message(m) => TypeRef::Message(MessageRef::from(
                    cache
                        .type_by_full_name(&format!("{}.{}", self_data.full_name, m.name))
                        .expect("Existing type wasn't added to the cache"),
                )),
                InnerTypeBuilder::Enum(e) => TypeRef::Enum(EnumRef::from(
                    cache
                        .type_by_full_name(&format!("{}.{}", self_data.full_name, e.name))
                        .expect("Existing type wasn't added to the cache"),
                )),
            })
            .collect();

        let mut fields: Vec<_> = self
            .fields
            .into_iter()
            .map(|field| field.build(self_data, cache, None))
            .collect::<Result<_, _>>()?;

        let mut oneofs: Vec<_> = self
            .oneofs
            .into_iter()
            .enumerate()
            .map(|(idx, oneof)| {
                let oneof_ref = OneofRef(InternalRef(idx));
                let mut new_fields: Vec<_> = oneof
                    .fields
                    .into_iter()
                    .map(|field| field.build(self_data, cache, Some(oneof_ref)))
                    .collect::<Result<_, _>>()?;
                fields.append(&mut new_fields);
                Ok(Oneof {
                    name: oneof.name,
                    self_ref: oneof_ref,
                    options: oneof.options,
                    fields: vec![],
                })
            })
            .collect::<Result<_, _>>()?;

        // Sort the fields by number just for sanity.
        let fields: BTreeMap<u64, MessageField> =
            fields.into_iter().map(|f| (f.number, f)).collect();
        for (idx, oneof) in oneofs.iter_mut().enumerate() {
            oneof.fields = fields
                .iter()
                .filter_map(
                    |(num, f)| match f.oneof == Some(OneofRef(InternalRef(idx))) {
                        true => Some(*num),
                        false => None,
                    },
                )
                .collect();
        }

        let parent = cache.parent_type(&self_data.idx_path);
        let fields_by_name = fields
            .iter()
            .map(|(i, f)| (f.name.to_string(), *i))
            .collect();

        Ok(MessageInfo {
            name: self.name,
            full_name: self_data.full_name.clone(),
            parent,
            self_ref: MessageRef(InternalRef(self_data.final_idx)),
            inner_types,
            oneofs,
            fields,
            fields_by_name,
        })
    }
}

impl InnerTypeBuilder {
    fn clone_name(&self) -> InnerTypeBuilder {
        match self {
            InnerTypeBuilder::Message(m) => InnerTypeBuilder::Message(MessageBuilder {
                name: m.name.clone(),
                ..Default::default()
            }),
            InnerTypeBuilder::Enum(e) => InnerTypeBuilder::Enum(EnumBuilder {
                name: e.name.clone(),
                ..Default::default()
            }),
        }
    }
}

impl FieldBuilder {
    fn build(
        self,
        self_data: &CacheData,
        cache: &BuildCache,
        oneof: Option<OneofRef>,
    ) -> Result<MessageField, ParseError> {
        let multiplicity = resolve_multiplicity(self.multiplicity, &self.field_type, &self.options);
        Ok(MessageField {
            name: self.name,
            number: self.number,
            multiplicity,
            field_type: self.field_type.build(self_data, cache)?,
            oneof,
            options: self.options,
        })
    }
}

fn resolve_multiplicity(
    proto_multiplicity: Multiplicity,
    field_type: &FieldTypeBuilder,
    options: &[ProtoOption],
) -> Multiplicity {
    // If this isn't a repeated field, the multiplicity follows the proto one (single or optional).
    if proto_multiplicity != Multiplicity::Repeated {
        return proto_multiplicity;
    }

    // Repeated field.
    match field_type {
        // Non-scalar fields are always repeated.
        FieldTypeBuilder::Unknown(..) => return Multiplicity::Repeated,
        FieldTypeBuilder::Builtin(vt) if vt.wire_type() == 2 => return Multiplicity::Repeated,

        // Scalar field.
        _ => {}
    }

    // Check the options.
    if let Some(opt) = options.iter().find(|o| o.name == "packed") {
        return match opt.value {
            Constant::Bool(true) => Multiplicity::RepeatedPacked,
            _ => Multiplicity::Repeated,
        };
    }

    Multiplicity::RepeatedPacked
}

impl FieldTypeBuilder {
    fn build(self, self_data: &CacheData, cache: &BuildCache) -> Result<ValueType, ParseError> {
        Ok(match self {
            FieldTypeBuilder::Builtin(vt) => vt,
            FieldTypeBuilder::Unknown(s) => {
                let t = cache
                    .resolve_type(&s, &self_data.full_name)
                    .ok_or_else(|| ParseError::TypeNotFound {
                        name: s,
                        context: self_data.full_name.to_string(),
                    })?;

                match t.item_type {
                    ItemType::Message => ValueType::Message(MessageRef(InternalRef(t.final_idx))),
                    ItemType::Enum => ValueType::Enum(EnumRef(InternalRef(t.final_idx))),
                    _ => unreachable!("Service as field type"),
                }
            }
        })
    }
}

impl InnerTypeBuilder {
    fn populate<'a>(
        &'a self,
        cache: &mut BuildCache,
        path: &mut Vec<&'a str>,
        idx: &mut Vec<usize>,
    ) -> Result<(), ParseError> {
        match self {
            InnerTypeBuilder::Message(m) => m.populate(cache, path, idx),
            InnerTypeBuilder::Enum(e) => e.populate(cache, path, idx),
        }
    }

    fn take_type(&mut self, idx: &[usize]) -> ProtobufTypeBuilder {
        match self {
            InnerTypeBuilder::Message(m) => m.take_type(idx),
            InnerTypeBuilder::Enum(e) => e.take_type(idx),
        }
    }
}

impl EnumBuilder {
    /// Lists types found in this message builder recursively into the build cache.
    ///
    /// On error the `path` and `idx` will be left in an undefined state.
    fn populate<'a>(
        &'a self,
        cache: &mut BuildCache,
        path: &mut Vec<&'a str>,
        idx: &mut Vec<usize>,
    ) -> Result<(), ParseError> {
        path.push(&self.name);
        let full_name = path.join(".");
        let cache_idx = cache.types.len();
        if cache
            .items
            .insert(full_name.clone(), (ItemType::Enum, cache_idx))
            .is_some()
        {
            return Err(ParseError::DuplicateType {
                name: path.join("."),
            });
        }
        path.pop();

        cache.types.push(CacheData {
            item_type: ItemType::Enum,
            full_name,
            idx_path: idx.clone(),
            final_idx: cache_idx,
        });

        Ok(())
    }

    fn build(self, self_data: &CacheData, cache: &BuildCache) -> Result<EnumInfo, ParseError> {
        let fields_by_name = self
            .fields
            .iter()
            .map(|f| (f.name.clone(), f.value))
            .collect();
        let fields_by_value = self.fields.into_iter().map(|f| (f.value, f)).collect();

        let parent = cache.parent_type(&self_data.idx_path);

        Ok(EnumInfo {
            name: self.name,
            full_name: self_data.full_name.to_string(),
            self_ref: EnumRef(InternalRef(self_data.final_idx)),
            parent,
            fields_by_value,
            fields_by_name,
        })
    }

    fn take_type(&mut self, idx: &[usize]) -> ProtobufTypeBuilder {
        if !idx.is_empty() {
            panic!("Trying to take an inner type from an enum");
        }

        ProtobufTypeBuilder::Enum(std::mem::take(self))
    }
}

impl ServiceBuilder {
    /// Lists types found in this message builder recursively into the build cache.
    ///
    /// On error the `path` and `idx` will be left in an undefined state.
    fn populate<'a>(
        &'a self,
        cache: &mut BuildCache,
        path: &mut Vec<&'a str>,
        idx: &mut Vec<usize>,
    ) -> Result<(), ParseError> {
        path.push(&self.name);
        let full_name = path.join(".");
        let cache_idx = cache.services.len();
        if let Some(..) = cache
            .items
            .insert(full_name.clone(), (ItemType::Service, cache_idx))
        {
            return Err(ParseError::DuplicateType {
                name: path.join("."),
            });
        }
        path.pop();

        cache.services.push(CacheData {
            item_type: ItemType::Service,
            full_name,
            idx_path: idx.clone(),
            final_idx: cache_idx,
        });

        Ok(())
    }

    fn build(self, self_data: &CacheData, cache: &BuildCache) -> Result<Service, ParseError> {
        let rpcs: Vec<_> = self
            .rpcs
            .into_iter()
            .map(|rpc| rpc.build(self_data, cache))
            .collect::<Result<_, _>>()?;
        let rpcs_by_name = rpcs
            .iter()
            .enumerate()
            .map(|(idx, rpc)| (rpc.name.to_string(), idx))
            .collect();

        let parent = match cache.parent_type(&self_data.idx_path) {
            TypeParent::Package(p) => p,
            _ => panic!("Service inside a message"),
        };

        Ok(Service {
            name: self.name,
            self_ref: ServiceRef(InternalRef(self_data.final_idx)),
            parent,
            full_name: self_data.full_name.clone(),
            rpcs,
            rpcs_by_name,
            options: vec![],
        })
    }
}

impl RpcBuilder {
    fn build(self, self_data: &CacheData, cache: &BuildCache) -> Result<Rpc, ParseError> {
        Ok(Rpc {
            name: self.name,
            input: self.input.build(self_data, cache)?,
            output: self.output.build(self_data, cache)?,
            options: vec![],
        })
    }
}

impl RpcArgBuilder {
    fn build(self, rpc_data: &CacheData, cache: &BuildCache) -> Result<RpcArg, ParseError> {
        // Fetch the type data from the cache so we can figure out the type reference.
        let self_data = match cache.resolve_type(&self.message, &rpc_data.full_name) {
            Some(data) => data,
            None => {
                return Err(ParseError::TypeNotFound {
                    name: self.message,
                    context: rpc_data.full_name.clone(),
                })
            }
        };

        // All rpc input/output types must be messages.
        if self_data.item_type != ItemType::Message {
            return Err(ParseError::InvalidTypeKind {
                type_name: self.message,
                context: "service input/output",
                expected: ItemType::Message,
                actual: self_data.item_type,
            });
        }

        let message = MessageRef(InternalRef(self_data.final_idx));
        Ok(RpcArg {
            stream: self.stream,
            message,
        })
    }
}

impl MessageRef {
    fn from(data: &CacheData) -> Self {
        if data.item_type != ItemType::Message {
            panic!("Trying to create MessageRef for {:?}", data.item_type);
        }
        MessageRef(InternalRef(data.final_idx))
    }
}

impl EnumRef {
    fn from(data: &CacheData) -> Self {
        if data.item_type != ItemType::Enum {
            panic!("Trying to create EnumRef for {:?}", data.item_type);
        }
        EnumRef(InternalRef(data.final_idx))
    }
}

#[derive(Default)]
struct BuildCache {
    items: BTreeMap<String, (ItemType, usize)>,
    items_by_idx: BTreeMap<Vec<usize>, (ItemType, usize)>,
    types: Vec<CacheData>,
    services: Vec<CacheData>,
}

struct CacheData {
    item_type: ItemType,
    idx_path: Vec<usize>,
    final_idx: usize,
    full_name: String,
}

impl BuildCache {
    fn resolve_type(&self, relative_name: &str, mut current_path: &str) -> Option<&CacheData> {
        if let Some(absolute) = relative_name.strip_prefix('.') {
            return self.type_by_full_name(absolute);
        }

        loop {
            let lookup: Cow<str> = match current_path.is_empty() {
                true => relative_name.into(),
                false => format!("{}.{}", current_path, relative_name).into(),
            };

            if let Some(t) = self.type_by_full_name(&lookup) {
                return Some(t);
            }

            if current_path.is_empty() {
                return None;
            }

            match current_path.rfind('.') {
                Some(i) => {
                    let (start, _) = current_path.split_at(i);
                    current_path = start;
                }
                None => {
                    current_path = "";
                }
            }
        }
    }

    fn parent_type(&self, current: &[usize]) -> TypeParent {
        match current.len() {
            0 | 1 => panic!("Empty type ID path"),
            2 => TypeParent::Package(PackageRef(InternalRef(current[0]))),
            _ => self
                .type_by_idx_path(&current[..current.len() - 1])
                .map(|data| TypeParent::Message(MessageRef(InternalRef(data.final_idx))))
                .unwrap_or_else(|| panic!("Parent type not found: {:?}", current)),
        }
    }

    fn type_by_full_name(&self, full_name: &str) -> Option<&CacheData> {
        self.items
            .get(full_name)
            .and_then(|(ty, i)| self.type_by_idx(*ty, *i))
    }

    fn type_by_idx_path(&self, idx: &[usize]) -> Option<&CacheData> {
        self.items_by_idx
            .get(idx)
            .and_then(|(ty, i)| self.type_by_idx(*ty, *i))
    }

    fn type_by_idx(&self, item_type: ItemType, idx: usize) -> Option<&CacheData> {
        match item_type {
            ItemType::Message => self.types.get(idx),
            ItemType::Enum => self.types.get(idx),
            ItemType::Service => self.services.get(idx),
        }
    }
}
