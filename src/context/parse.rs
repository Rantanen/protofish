use bytes::{BufMut, Bytes, BytesMut};
use pest::{
    iterators::{Pair, Pairs},
    Parser,
};

use super::builder::*;
use super::*;

#[derive(pest_derive::Parser)]
#[grammar = "proto.pest"]
struct ProtoParser;

impl Context
{
    /// Parses the files and creates a decoding context.
    pub fn parse<T, S>(files: T) -> Result<Self, ParseError>
    where
        T: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let builder = ContextBuilder {
            packages: files
                .into_iter()
                .map(|f| PackageBuilder::parse_str(f.as_ref()))
                .collect::<Result<_, _>>()?,
        };

        builder.build()
    }
}

impl PackageBuilder
{
    pub fn parse_str(input: &str) -> Result<Self, ParseError>
    {
        let pairs = ProtoParser::parse(Rule::proto, input)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(SyntaxError {})?;

        let mut current_package = PackageBuilder::default();
        for pair in pairs {
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::syntax => {}
                    Rule::topLevelDef => current_package
                        .types
                        .push(ProtobufItemBuilder::parse(inner)),
                    Rule::import => {}
                    Rule::package => {
                        current_package.name =
                            Some(inner.into_inner().next().unwrap().as_str().to_string())
                    }
                    Rule::option => {}
                    Rule::EOI => {}
                    r => unreachable!("{:?}: {:?}", r, inner),
                }
            }
        }

        Ok(current_package)
    }
}

impl ProtobufItemBuilder
{
    pub fn parse(p: Pair<Rule>) -> Self
    {
        let pair = p.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::message => {
                ProtobufItemBuilder::Type(ProtobufTypeBuilder::Message(MessageBuilder::parse(pair)))
            }
            Rule::enum_ => {
                ProtobufItemBuilder::Type(ProtobufTypeBuilder::Enum(EnumBuilder::parse(pair)))
            }
            Rule::service => ProtobufItemBuilder::Service(ServiceBuilder::parse(pair)),
            r => unreachable!("{:?}: {:?}", r, pair),
        }
    }
}

impl MessageBuilder
{
    pub fn parse(p: Pair<Rule>) -> Self
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap().as_str().to_string();

        let mut fields = vec![];
        let mut oneofs = vec![];
        let mut inner_types = vec![];
        let mut options = vec![];
        let body = inner.next().unwrap();
        for p in body.into_inner() {
            match p.as_rule() {
                Rule::field => fields.push(FieldBuilder::parse(p)),
                Rule::enum_ => inner_types.push(InnerTypeBuilder::Enum(EnumBuilder::parse(p))),
                Rule::message => {
                    inner_types.push(InnerTypeBuilder::Message(MessageBuilder::parse(p)))
                }
                Rule::option => options.push(ProtoOption::parse(p)),
                Rule::oneof => oneofs.push(OneofBuilder::parse(p)),
                Rule::mapField => {
                    let (entry, field) = parse_map_field(p);
                    inner_types.push(InnerTypeBuilder::Message(entry));
                    fields.push(field);
                }
                Rule::reserved => {} // We don't need to care about reserved field numbers.
                Rule::emptyStatement => {}
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }

        MessageBuilder {
            name,
            fields,
            oneofs,
            inner_types,
            options,
        }
    }
}

impl EnumBuilder
{
    fn parse(p: Pair<Rule>) -> EnumBuilder
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap().as_str().to_string();

        let mut fields = vec![];
        let mut options = vec![];
        let body = inner.next().unwrap();
        for p in body.into_inner() {
            match p.as_rule() {
                Rule::enumField => {
                    let mut inner = p.into_inner();
                    fields.push(EnumField {
                        name: inner.next().unwrap().as_str().to_string(),
                        value: parse_int_literal(inner.next().unwrap()),
                        options: ProtoOption::parse_options(inner),
                    })
                }
                Rule::option => options.push(ProtoOption::parse(p)),
                Rule::emptyStatement => {}
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }

        EnumBuilder {
            name,
            fields,
            options,
        }
    }
}

impl ServiceBuilder
{
    pub fn parse(p: Pair<Rule>) -> Self
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap();
        let mut rpcs = vec![];
        let mut options = vec![];
        for p in inner {
            match p.as_rule() {
                Rule::option => options.push(ProtoOption::parse(p)),
                Rule::rpc => rpcs.push(RpcBuilder::parse(p)),
                Rule::emptyStatement => {}
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }

        ServiceBuilder {
            name: name.as_str().to_string(),
            rpcs,
            options,
        }
    }
}

impl FieldBuilder
{
    pub fn parse(p: Pair<Rule>) -> Self
    {
        let mut inner = p.into_inner();
        let multiplicity = match inner.next().unwrap().into_inner().next() {
            Some(t) => {
                let field_multiplicity = t.into_inner().next().unwrap().as_rule();
                match field_multiplicity {
                    Rule::optional => Multiplicity::Optional,
                    Rule::repeated => Multiplicity::Repeated,
                    r => unreachable!("{:?}: {:?}", r, field_multiplicity),
                }
            }
            None => Multiplicity::Single,
        };
        let field_type = parse_field_type(inner.next().unwrap().as_str());
        let name = inner.next().unwrap().as_str().to_string();
        let number = parse_uint_literal(inner.next().unwrap());

        let options = match inner.next() {
            Some(p) => ProtoOption::parse_options(p.into_inner()),
            None => vec![],
        };

        FieldBuilder {
            multiplicity,
            field_type,
            name,
            number,
            options,
        }
    }

    pub fn parse_oneof(p: Pair<Rule>) -> Self
    {
        let mut inner = p.into_inner();
        let field_type = parse_field_type(inner.next().unwrap().as_str());
        let name = inner.next().unwrap().as_str().to_string();
        let number = parse_uint_literal(inner.next().unwrap());

        let options = match inner.next() {
            Some(p) => ProtoOption::parse_options(p.into_inner()),
            None => vec![],
        };

        FieldBuilder {
            multiplicity: Multiplicity::Single,
            field_type,
            name,
            number,
            options,
        }
    }
}

impl OneofBuilder
{
    pub fn parse(p: Pair<Rule>) -> Self
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap().as_str().to_string();
        let mut options = Vec::new();
        let mut fields = vec![];
        for p in inner {
            match p.as_rule() {
                Rule::option => options.push(ProtoOption::parse(p)),
                Rule::oneofField => fields.push(FieldBuilder::parse_oneof(p)),
                Rule::emptyStatement => {}
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }
        OneofBuilder {
            name,
            fields,
            options,
        }
    }
}

/// Parses a `map<K, V> name = N;` field into the form it has on the wire.
///
/// Maps are encoded as if they were declared as:
///
/// ```proto
/// message NameEntry {
///     K key = 1;
///     V value = 2;
/// }
/// repeated NameEntry name = N;
/// ```
///
/// See https://protobuf.dev/programming-guides/proto3/#backwards. The entry message is named the
/// same way as `protoc` names it, so the types match the ones in descriptors that `protoc` and
/// gRPC server reflection produce.
fn parse_map_field(p: Pair<Rule>) -> (MessageBuilder, FieldBuilder)
{
    let mut inner = p.into_inner();
    let key_type = parse_field_type(inner.next().unwrap().as_str());
    let value_type = parse_field_type(inner.next().unwrap().as_str());
    let name = inner.next().unwrap().as_str().to_string();
    let number = parse_uint_literal(inner.next().unwrap());
    let options = match inner.next() {
        Some(p) => ProtoOption::parse_options(p.into_inner()),
        None => vec![],
    };

    let entry_name = map_entry_name(&name);
    let entry = MessageBuilder {
        name: entry_name.clone(),
        fields: vec![
            FieldBuilder {
                multiplicity: Multiplicity::Single,
                field_type: key_type,
                name: "key".to_string(),
                number: 1,
                options: vec![],
            },
            FieldBuilder {
                multiplicity: Multiplicity::Single,
                field_type: value_type,
                name: "value".to_string(),
                number: 2,
                options: vec![],
            },
        ],
        ..Default::default()
    };
    let field = FieldBuilder {
        multiplicity: Multiplicity::Repeated,
        field_type: FieldTypeBuilder::Unknown(entry_name),
        name,
        number,
        options,
    };

    (entry, field)
}

/// Name of the entry message for a map field, the same as `protoc` uses: the field name in
/// PascalCase followed by `Entry`. For example `string_values` becomes `StringValuesEntry`.
fn map_entry_name(field_name: &str) -> String
{
    let mut name = String::with_capacity(field_name.len() + 5);
    let mut capitalize_next = true;
    for c in field_name.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            name.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            name.push(c);
        }
    }
    name.push_str("Entry");
    name
}

fn parse_field_type(t: &str) -> FieldTypeBuilder
{
    FieldTypeBuilder::Builtin(match t {
        "double" => ValueType::Double,
        "float" => ValueType::Float,
        "int32" => ValueType::Int32,
        "int64" => ValueType::Int64,
        "uint32" => ValueType::UInt32,
        "uint64" => ValueType::UInt64,
        "sint32" => ValueType::SInt32,
        "sint64" => ValueType::SInt64,
        "fixed32" => ValueType::Fixed32,
        "fixed64" => ValueType::Fixed64,
        "sfixed32" => ValueType::SFixed32,
        "sfixed64" => ValueType::SFixed64,
        "bool" => ValueType::Bool,
        "string" => ValueType::String,
        "bytes" => ValueType::Bytes,
        _ => return FieldTypeBuilder::Unknown(t.to_string()),
    })
}

impl RpcBuilder
{
    pub fn parse(p: Pair<Rule>) -> Self
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap();

        let input = RpcArgBuilder::parse(inner.next().unwrap());
        let output = RpcArgBuilder::parse(inner.next().unwrap());

        let mut options = vec![];
        for p in inner {
            match p.as_rule() {
                Rule::option => options.push(ProtoOption::parse(p)),
                Rule::emptyStatement => {}
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }

        RpcBuilder {
            name: name.as_str().to_string(),
            input,
            output,
            options,
        }
    }
}

impl RpcArgBuilder
{
    pub fn parse(p: Pair<Rule>) -> Self
    {
        let mut inner = p.into_inner();
        RpcArgBuilder {
            stream: inner.next().unwrap().into_inner().next().is_some(),
            message: inner.next().unwrap().as_str().to_string(),
        }
    }
}

pub fn parse_uint_literal(p: Pair<Rule>) -> u64
{
    match p.as_rule() {
        Rule::fieldNumber => parse_uint_literal(p.into_inner().next().unwrap()),
        Rule::intLit => {
            let mut inner = p.into_inner();
            let lit = inner.next().unwrap();
            match lit.as_rule() {
                Rule::decimalLit => str::parse(lit.as_str()).unwrap(),
                Rule::octalLit => u64::from_str_radix(&lit.as_str()[1..], 8).unwrap(),
                Rule::hexLit => u64::from_str_radix(&lit.as_str()[2..], 16).unwrap(),
                r => unreachable!("{:?}: {:?}", r, lit),
            }
        }
        r => unreachable!("{:?}: {:?}", r, p),
    }
}

pub fn parse_int_literal(p: Pair<Rule>) -> i64
{
    match p.as_rule() {
        Rule::intLit => {
            let mut inner = p.into_inner();
            let sign = inner.next().unwrap();
            let (sign, lit) = match sign.as_rule() {
                Rule::sign if sign.as_str() == "-" => (-1, inner.next().unwrap()),
                Rule::sign if sign.as_str() == "+" => (1, inner.next().unwrap()),
                _ => (1, sign),
            };
            match lit.as_rule() {
                Rule::decimalLit => sign * str::parse::<i64>(lit.as_str()).unwrap(),
                Rule::octalLit => sign * i64::from_str_radix(lit.as_str(), 8).unwrap(),
                Rule::hexLit => sign * i64::from_str_radix(&lit.as_str()[2..], 16).unwrap(),
                r => unreachable!("{:?}: {:?}", r, lit),
            }
        }
        r => unreachable!("{:?}: {:?}", r, p),
    }
}

pub fn parse_float_literal(p: Pair<Rule>) -> f64
{
    match p.as_rule() {
        Rule::floatLit => p.as_str().parse::<f64>().unwrap(),
        r => unreachable!("{:?}: {:?}", r, p),
    }
}

impl ProtoOption
{
    fn parse(p: Pair<Rule>) -> Self
    {
        let mut inner = p.into_inner();
        Self {
            name: parse_ident(inner.next().unwrap()),
            value: Constant::parse(inner.next().unwrap()),
        }
    }

    fn parse_options(pairs: Pairs<Rule>) -> Vec<Self>
    {
        pairs
            .map(|p| match p.as_rule() {
                Rule::fieldOption => Self::parse(p),
                Rule::enumValueOption => Self::parse(p),
                Rule::option => Self::parse(p),
                r => unreachable!("{:?}: {:?}", r, p),
            })
            .collect()
    }
}

impl Constant
{
    fn parse(p: Pair<Rule>) -> Self
    {
        let p = p.into_inner().next().unwrap();
        match p.as_rule() {
            Rule::fullIdent => Constant::Ident(parse_ident(p)),
            Rule::intLit => Constant::Integer(parse_int_literal(p)),
            Rule::floatLit => Constant::Float(parse_float_literal(p)),
            Rule::strLit => Constant::String(parse_string_literal(p)),
            Rule::boolLit => Constant::Bool(p.as_str() == "true"),
            r => unreachable!("{:?}: {:?}", r, p),
        }
    }
}

fn parse_ident(p: Pair<Rule>) -> String
{
    let mut ident = vec![];
    let mut inner = p.into_inner();

    let first = inner.next().unwrap();
    match first.as_rule() {
        Rule::ident => ident.push(first.as_str().to_string()),
        Rule::fullIdent => ident.push(format!("({})", parse_ident(first))),
        r => unreachable!("{:?}: {:?}", r, first),
    }

    for other in inner {
        match other.as_rule() {
            Rule::ident => ident.push(other.as_str().to_string()),
            r => unreachable!("{:?}: {:?}", r, other),
        }
    }

    ident.join(".")
}

fn parse_string_literal(s: Pair<Rule>) -> Bytes
{
    let inner = s.into_inner();
    let mut output = BytesMut::new();
    for c in inner {
        let c = c.into_inner().next().unwrap();
        match c.as_rule() {
            Rule::hexEscape => {
                output.put_u8(
                    u8::from_str_radix(c.into_inner().next().unwrap().as_str(), 16).unwrap(),
                );
            }
            Rule::octEscape => {
                output.put_u8(
                    u8::from_str_radix(c.into_inner().next().unwrap().as_str(), 8).unwrap(),
                );
            }
            Rule::charEscape => match c.into_inner().next().unwrap().as_str() {
                "a" => output.put_u8(0x07),
                "b" => output.put_u8(0x08),
                "f" => output.put_u8(0x0C),
                "n" => output.put_u8(0x0A),
                "r" => output.put_u8(0x0D),
                "t" => output.put_u8(0x09),
                "v" => output.put_u8(0x0B),
                "\\" => output.put_u8(0x5C),
                "\'" => output.put_u8(0x27),
                "\"" => output.put_u8(0x22),
                o => unreachable!("Invalid escape sequence \\{}", o),
            },
            Rule::anyChar => output.put(c.as_str().as_ref()),
            r => unreachable!("{:?}: {:?}", r, c),
        }
    }
    output.freeze()
}

#[cfg(test)]
mod test
{
    use super::*;

    #[test]
    fn map_entry_names_match_protoc()
    {
        for (field, entry) in [
            ("values", "ValuesEntry"),
            ("string_values", "StringValuesEntry"),
            ("x", "XEntry"),
            ("tags_by_id", "TagsByIdEntry"),
            ("camelCase", "CamelCaseEntry"),
            ("_leading", "LeadingEntry"),
            ("double__underscore", "DoubleUnderscoreEntry"),
            ("value_2", "Value2Entry"),
        ] {
            assert_eq!(map_entry_name(field), entry, "{}", field);
        }
    }

    #[test]
    fn empty()
    {
        assert_eq!(
            PackageBuilder::parse_str(
                r#"
                syntax = "proto3";
            "#
            )
            .unwrap(),
            PackageBuilder::default(),
        );
    }

    #[test]
    fn package()
    {
        assert_eq!(
            PackageBuilder::parse_str(
                r#"
                syntax = "proto3";
                package Test;
            "#
            )
            .unwrap(),
            PackageBuilder {
                name: Some("Test".to_string()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn bom()
    {
        assert_eq!(
            PackageBuilder::parse_str(&format!(
                "\u{FEFF}{}",
                r#"
                syntax = "proto3";
                package Test;
            "#
            ))
            .unwrap(),
            PackageBuilder {
                name: Some("Test".to_string()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn message()
    {
        assert_eq!(
            PackageBuilder::parse_str(
                r#"
                syntax = "proto3";

                message MyMessage {
                    int32 value = 1;
                }
            "#
            )
            .unwrap(),
            PackageBuilder {
                types: vec![ProtobufItemBuilder::Type(ProtobufTypeBuilder::Message(
                    MessageBuilder {
                        name: "MyMessage".to_string(),
                        fields: vec![FieldBuilder {
                            multiplicity: Multiplicity::Single,
                            field_type: FieldTypeBuilder::Builtin(ValueType::Int32),
                            name: "value".to_string(),
                            number: 1,
                            options: vec![],
                        }],
                        ..Default::default()
                    }
                )),],
                ..Default::default()
            }
        );
    }

    #[test]
    fn pbenum()
    {
        assert_eq!(
            PackageBuilder::parse_str(
                r#"
                syntax = "proto3";

                enum MyEnum {
                    a = 1;
                    b = -1;
                }
            "#
            )
            .unwrap(),
            PackageBuilder {
                types: vec![ProtobufItemBuilder::Type(ProtobufTypeBuilder::Enum(
                    EnumBuilder {
                        name: "MyEnum".to_string(),
                        fields: vec![
                            EnumField {
                                name: "a".to_string(),
                                value: 1,
                                options: vec![],
                            },
                            EnumField {
                                name: "b".to_string(),
                                value: -1,
                                options: vec![],
                            }
                        ],
                        ..Default::default()
                    }
                )),],
                ..Default::default()
            }
        );
    }

    #[test]
    fn service()
    {
        assert_eq!(
            PackageBuilder::parse_str(
                r#"
                syntax = "proto3";

                service MyService {
                    rpc function( Foo ) returns ( stream Bar );
                }
            "#
            )
            .unwrap(),
            PackageBuilder {
                types: vec![ProtobufItemBuilder::Service(ServiceBuilder {
                    name: "MyService".to_string(),
                    rpcs: vec![RpcBuilder {
                        name: "function".to_string(),
                        input: RpcArgBuilder {
                            stream: false,
                            message: "Foo".to_string(),
                        },
                        output: RpcArgBuilder {
                            stream: true,
                            message: "Bar".to_string(),
                        },
                        ..Default::default()
                    },],
                    ..Default::default()
                }),],
                ..Default::default()
            }
        );
    }

    #[test]
    fn options()
    {
        assert_eq!(
            PackageBuilder::parse_str(
                r#"
                syntax = "proto3";

                message Message {
                    option mOption = "foo";
                    uint32 field = 1 [ fOption = bar ];
                }

                enum Enum {
                    value = 1 [ (a.b).c = 1, o2 = 2 ];
                    option eOption = "banana";
                }

                service MyService {
                    rpc function( Foo ) returns ( stream Bar ) { option o = true; }
                    option sOption = "bar";
                }
            "#
            )
            .unwrap(),
            PackageBuilder {
                types: vec![
                    ProtobufItemBuilder::Type(ProtobufTypeBuilder::Message(MessageBuilder {
                        name: "Message".to_string(),
                        fields: vec![FieldBuilder {
                            multiplicity: Multiplicity::Single,
                            field_type: FieldTypeBuilder::Builtin(ValueType::UInt32),
                            name: "field".to_string(),
                            number: 1,
                            options: vec![ProtoOption {
                                name: "fOption".to_string(),
                                value: Constant::Ident("bar".to_string()),
                            }],
                        }],
                        options: vec![ProtoOption {
                            name: "mOption".to_string(),
                            value: Constant::String(Bytes::from_static(b"foo")),
                        }],
                        ..Default::default()
                    })),
                    ProtobufItemBuilder::Type(ProtobufTypeBuilder::Enum(EnumBuilder {
                        name: "Enum".to_string(),
                        fields: vec![EnumField {
                            name: "value".to_string(),
                            value: 1,
                            options: vec![
                                ProtoOption {
                                    name: "(a.b).c".to_string(),
                                    value: Constant::Integer(1),
                                },
                                ProtoOption {
                                    name: "o2".to_string(),
                                    value: Constant::Integer(2),
                                }
                            ],
                        }],
                        options: vec![ProtoOption {
                            name: "eOption".to_string(),
                            value: Constant::String(Bytes::from_static(b"banana")),
                        }],
                        ..Default::default()
                    })),
                    ProtobufItemBuilder::Service(ServiceBuilder {
                        name: "MyService".to_string(),
                        rpcs: vec![RpcBuilder {
                            name: "function".to_string(),
                            input: RpcArgBuilder {
                                stream: false,
                                message: "Foo".to_string(),
                            },
                            output: RpcArgBuilder {
                                stream: true,
                                message: "Bar".to_string(),
                            },
                            options: vec![ProtoOption {
                                name: "o".to_string(),
                                value: Constant::Bool(true),
                            }]
                        },],
                        options: vec![ProtoOption {
                            name: "sOption".to_string(),
                            value: Constant::String(Bytes::from_static(b"bar")),
                        }]
                    }),
                ],
                ..Default::default()
            }
        );
    }

    #[test]
    fn parse_string_vec()
    {
        let _ = Context::parse(&["foo", "bar"]);
        let _ = Context::parse(vec!["foo", "bar"]);
        let _ = Context::parse(vec!["foo".to_string(), "bar".to_string()]);
    }
}
