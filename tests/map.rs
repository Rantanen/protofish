use bytes::BufMut;
use protofish::context::{Context, MessageInfo, Multiplicity, ValueType};
use protofish::decode::{FieldValue, MessageValue, Value};

const PROTO: &str = r#"
    syntax = "proto3";
    package test;

    enum Color {
        COLOR_UNSPECIFIED = 0;
        COLOR_RED = 1;
    }

    message Item {
        message Tag {
            string label = 1;
        }

        message Inner {
            // `Tag` resolves to `Item.Tag` from within `Inner.TagsEntry`.
            map<int32, Tag> tags = 1;
        }

        map<string, int64> counts = 1;
        map<int32, Tag> tags_by_id = 2 [deprecated = true];
        map<string, Color> color_by_name = 3;
        string name = 4;
        Inner inner = 5;
    }
"#;

fn assert_map_field(
    context: &Context,
    msg: &MessageInfo,
    field_name: &str,
    entry_name: &str,
    key: ValueType,
    value: ValueType,
)
{
    let field = msg.get_field_by_name(field_name).expect(field_name);
    assert_eq!(field.multiplicity, Multiplicity::Repeated, "{}", field_name);

    let entry_full_name = format!("{}.{}", msg.full_name, entry_name);
    let entry = context
        .get_message(&entry_full_name)
        .expect(&entry_full_name);
    assert_eq!(field.field_type, ValueType::Message(entry.self_ref));
    assert!(msg
        .inner_types
        .contains(&protofish::context::TypeRef::Message(entry.self_ref)));

    let fields: Vec<_> = entry
        .iter_fields()
        .map(|f| {
            (
                f.number,
                f.name.as_str(),
                f.field_type.clone(),
                f.multiplicity.clone(),
            )
        })
        .collect();
    assert_eq!(
        fields,
        [
            (1, "key", key, Multiplicity::Single),
            (2, "value", value, Multiplicity::Single),
        ],
        "{}",
        entry_full_name
    );
}

#[test]
fn map_fields_are_repeated_entry_messages()
{
    let context = Context::parse([PROTO]).unwrap();
    let item = context.get_message("test.Item").unwrap();
    let tag = context.get_message("test.Item.Tag").unwrap().self_ref;
    let color = match context.get_type("test.Color").unwrap() {
        protofish::context::TypeInfo::Enum(e) => e.self_ref,
        _ => panic!("Color is an enum"),
    };

    assert_map_field(
        &context,
        item,
        "counts",
        "CountsEntry",
        ValueType::String,
        ValueType::Int64,
    );
    assert_map_field(
        &context,
        item,
        "tags_by_id",
        "TagsByIdEntry",
        ValueType::Int32,
        ValueType::Message(tag),
    );
    assert_map_field(
        &context,
        item,
        "color_by_name",
        "ColorByNameEntry",
        ValueType::String,
        ValueType::Enum(color),
    );

    let inner = context.get_message("test.Item.Inner").unwrap();
    assert_map_field(
        &context,
        inner,
        "tags",
        "TagsEntry",
        ValueType::Int32,
        ValueType::Message(tag),
    );

    // Field options stay on the map field.
    let tags_by_id = item.get_field_by_name("tags_by_id").unwrap();
    assert_eq!(tags_by_id.options.len(), 1);
    assert_eq!(tags_by_id.options[0].name, "deprecated");

    // Other fields are unaffected.
    assert_eq!(item.get_field(4).unwrap().name, "name");
    assert_eq!(item.get_field(5).unwrap().name, "inner");
}

fn put_len(payload: &mut bytes::BytesMut, tag: u8, data: &[u8])
{
    payload.put_u8(tag << 3 | 2);
    payload.put_u8(data.len() as u8);
    payload.put_slice(data);
}

#[test]
fn map_fields_decode_and_encode()
{
    let context = Context::parse([PROTO]).unwrap();
    let item = context.get_message("test.Item").unwrap();
    let counts_entry = context.get_message("test.Item.CountsEntry").unwrap();
    let tags_entry = context.get_message("test.Item.TagsByIdEntry").unwrap();
    let tag = context.get_message("test.Item.Tag").unwrap();

    // counts { "a": 1 }, counts { "b": 300 }, tags_by_id { 7: Tag { label: "x" } }
    let mut payload = bytes::BytesMut::new();
    put_len(&mut payload, 1, b"\x0a\x01a\x10\x01");
    put_len(&mut payload, 1, b"\x0a\x01b\x10\xac\x02");
    put_len(&mut payload, 2, b"\x08\x07\x12\x03\x0a\x01x");

    let value = item.decode(&payload, &context);
    let entry = |msg_ref, fields| {
        Value::Message(Box::new(MessageValue {
            msg_ref,
            garbage: None,
            fields,
        }))
    };
    assert_eq!(
        value,
        MessageValue {
            msg_ref: item.self_ref,
            garbage: None,
            fields: vec![
                FieldValue {
                    number: 1,
                    value: entry(
                        counts_entry.self_ref,
                        vec![
                            FieldValue {
                                number: 1,
                                value: Value::String("a".to_string()),
                            },
                            FieldValue {
                                number: 2,
                                value: Value::Int64(1),
                            },
                        ]
                    ),
                },
                FieldValue {
                    number: 1,
                    value: entry(
                        counts_entry.self_ref,
                        vec![
                            FieldValue {
                                number: 1,
                                value: Value::String("b".to_string()),
                            },
                            FieldValue {
                                number: 2,
                                value: Value::Int64(300),
                            },
                        ]
                    ),
                },
                FieldValue {
                    number: 2,
                    value: entry(
                        tags_entry.self_ref,
                        vec![
                            FieldValue {
                                number: 1,
                                value: Value::Int32(7),
                            },
                            FieldValue {
                                number: 2,
                                value: entry(
                                    tag.self_ref,
                                    vec![FieldValue {
                                        number: 1,
                                        value: Value::String("x".to_string()),
                                    }]
                                ),
                            },
                        ]
                    ),
                },
            ],
        }
    );

    assert_eq!(value.encode(&context), payload);
}

#[test]
fn map_entry_name_conflict_is_an_error()
{
    // protoc rejects this too: the map field generates a `CountsEntry` message.
    let result = Context::parse([r#"
        syntax = "proto3";
        message Item {
            message CountsEntry {}
            map<string, int64> counts = 1;
        }
    "#]);
    assert_eq!(
        result.unwrap_err().to_string(),
        "Duplicate type: Item.CountsEntry"
    );
}
