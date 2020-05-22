use pest::{iterators::Pair, Parser};

use super::builder::*;
use super::*;

#[derive(pest_derive::Parser)]
#[grammar = "proto.pest"]
struct ProtoParser;

impl Context
{
    /// Parses the files and creates a decoding context.
    pub fn parse(files: &[&str]) -> Result<Self>
    {
        let builder = ContextBuilder {
            packages: files
                .iter()
                .map(|f| PackageBuilder::parse_str(f))
                .collect::<Result<_, _>>()?,
        };

        builder.build()
    }
}

impl PackageBuilder
{
    pub fn parse_str(input: &str) -> Result<Self>
    {
        let pairs = ProtoParser::parse(Rule::proto, &input)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(ParseError {})?;

        let mut current_package = PackageBuilder::default();
        for pair in pairs {
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::syntax => {}
                    Rule::topLevelDef => current_package
                        .types
                        .push(ProtobufItemBuilder::parse(inner)?),
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
    pub fn parse(p: Pair<Rule>) -> Result<Self>
    {
        let pair = p.into_inner().next().unwrap();
        Ok(match pair.as_rule() {
            Rule::message => ProtobufItemBuilder::Type(ProtobufTypeBuilder::Message(
                MessageBuilder::parse(pair)?,
            )),
            Rule::enum_ => {
                ProtobufItemBuilder::Type(ProtobufTypeBuilder::Enum(EnumBuilder::parse(pair)?))
            }
            Rule::service => ProtobufItemBuilder::Service(ServiceBuilder::parse(pair)?),
            r => unreachable!("{:?}: {:?}", r, pair),
        })
    }
}

impl MessageBuilder
{
    pub fn parse(p: Pair<Rule>) -> Result<Self>
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap().as_str().to_string();

        let mut fields = vec![];
        let mut oneofs = vec![];
        let mut inner_types = vec![];
        let body = inner.next().unwrap();
        for p in body.into_inner() {
            match p.as_rule() {
                Rule::field => fields.push(FieldBuilder::parse(p)?),
                Rule::oneof => oneofs.push(OneofBuilder::parse(p)?),
                Rule::message => {
                    inner_types.push(InnerTypeBuilder::Message(MessageBuilder::parse(p)?))
                }
                Rule::enum_ => inner_types.push(InnerTypeBuilder::Enum(EnumBuilder::parse(p)?)),
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }

        Ok(MessageBuilder {
            name,
            fields,
            oneofs,
            inner_types,
        })
    }
}

impl EnumBuilder
{
    fn parse(p: Pair<Rule>) -> Result<EnumBuilder>
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap().as_str().to_string();

        let mut fields = vec![];
        let body = inner.next().unwrap();
        for p in body.into_inner() {
            match p.as_rule() {
                Rule::enumField => {
                    let mut inner = p.into_inner();
                    fields.push(EnumField {
                        name: inner.next().unwrap().as_str().to_string(),
                        value: parse_int_literal(inner.next().unwrap())?,
                    })
                }
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }

        Ok(EnumBuilder { name, fields })
    }
}

impl ServiceBuilder
{
    pub fn parse(p: Pair<Rule>) -> Result<Self>
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap();
        let mut rpc = vec![];
        let mut options = vec![];
        for p in inner {
            match p.as_rule() {
                Rule::option => options.push(parse_option(p)?),
                Rule::rpc => rpc.push(RpcBuilder::parse(p)?),
                Rule::emptyStatement => {}
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }

        Ok(ServiceBuilder {
            name: name.as_str().to_string(),
            rpcs: rpc,
            options: options,
        })
    }
}

impl FieldBuilder
{
    pub fn parse(p: Pair<Rule>) -> Result<Self>
    {
        let mut repeated = false;
        let mut type_ = "";
        let mut name = String::new();
        let mut number = 0;
        let mut options = Vec::new();
        for p in p.into_inner() {
            match p.as_rule() {
                Rule::repeated => repeated = true,
                Rule::type_ => type_ = p.as_str(),
                Rule::fieldName => name = p.as_str().to_string(),
                Rule::fieldNumber => number = parse_uint_literal(p)?,
                Rule::fieldOptions => options = parse_options(p)?,
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }
        let field_type = parse_field_type(type_);
        Ok(FieldBuilder {
            repeated,
            field_type,
            name,
            number,
            options,
        })
    }
}

impl OneofBuilder
{
    pub fn parse(p: Pair<Rule>) -> Result<Self>
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap().as_str().to_string();
        let mut options = Vec::new();
        let mut fields = vec![];
        for p in inner {
            match p.as_rule() {
                Rule::option => options.push(parse_option(p)?),
                Rule::oneofField => fields.push(FieldBuilder::parse(p)?),
                Rule::emptyStatement => {}
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }
        Ok(OneofBuilder {
            name,
            fields,
            options,
        })
    }
}

/*
pub fn parse_oneof_field(p: Pair<Rule>, oneof_idx: usize) -> Result<MessageField>
{
    let mut inner = p.into_inner();
    let field_type = parse_field_type(inner.next().unwrap().as_str());
    let name = inner.next().unwrap().as_str().to_string();
    let number = parse_uint_literal(inner.next().unwrap())?;
    let options = match inner.next() {
        Some(opt) => parse_options(opt)?,
        None => vec![],
    };

    Ok(MessageField {
        repeated: false,
        field_type,
        name,
        number,
        options,
        oneof: Some(oneof_idx),
    })
}
*/

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
    pub fn parse(p: Pair<Rule>) -> Result<Self>
    {
        let mut inner = p.into_inner();
        let name = inner.next().unwrap();

        let input = RpcArgBuilder::parse(inner.next().unwrap())?;
        let output = RpcArgBuilder::parse(inner.next().unwrap())?;

        let mut options = vec![];
        for p in inner {
            match p.as_rule() {
                Rule::option => options.push(parse_option(p)?),
                Rule::emptyStatement => {}
                r => unreachable!("{:?}: {:?}", r, p),
            }
        }

        Ok(RpcBuilder {
            name: name.as_str().to_string(),
            input,
            output,
            options,
        })
    }
}

impl RpcArgBuilder
{
    pub fn parse(p: Pair<Rule>) -> Result<Self>
    {
        let mut inner = p.into_inner();
        Ok(RpcArgBuilder {
            stream: inner.next().unwrap().into_inner().next().is_some(),
            message: inner.next().unwrap().as_str().to_string(),
        })
    }
}

pub fn parse_uint_literal(p: Pair<Rule>) -> Result<u64>
{
    match p.as_rule() {
        Rule::fieldNumber => parse_uint_literal(p.into_inner().next().unwrap()),
        Rule::intLit => {
            let mut inner = p.into_inner();
            let lit = inner.next().unwrap();
            Ok(match lit.as_rule() {
                Rule::decimalLit => u64::from_str_radix(lit.as_str(), 10).unwrap(),
                Rule::octalLit => u64::from_str_radix(&lit.as_str()[1..], 8).unwrap(),
                Rule::hexLit => u64::from_str_radix(&lit.as_str()[2..], 16).unwrap(),
                r => unreachable!("{:?}: {:?}", r, lit),
            })
        }
        r => unreachable!("{:?}: {:?}", r, p),
    }
}

pub fn parse_int_literal(p: Pair<Rule>) -> Result<i64>
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
            Ok(match lit.as_rule() {
                Rule::decimalLit => sign * i64::from_str_radix(lit.as_str(), 10).unwrap(),
                Rule::octalLit => sign * i64::from_str_radix(&lit.as_str(), 8).unwrap(),
                Rule::hexLit => sign * i64::from_str_radix(&lit.as_str()[2..], 16).unwrap(),
                r => unreachable!("{:?}: {:?}", r, lit),
            })
        }
        r => unreachable!("{:?}: {:?}", r, p),
    }
}

pub fn parse_options(_p: Pair<Rule>) -> Result<Vec<ProtoOption>>
{
    Ok(vec![])
}

pub fn parse_option(_p: Pair<Rule>) -> Result<ProtoOption>
{
    Ok(ProtoOption {})
}

#[cfg(test)]
mod test
{
    use super::builder::*;
    use super::*;

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
                            repeated: false,
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
                            },
                            EnumField {
                                name: "b".to_string(),
                                value: -1,
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
}
