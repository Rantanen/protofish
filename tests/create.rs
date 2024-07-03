use protofish::context::{
    Context, EnumField, EnumInfo, MessageField, MessageInfo, Oneof, Package, TypeParent, ValueType,
};

#[test]
fn create_context_by_hand() {
    let parsed_context = Context::parse(&[r#"
        syntax = "proto3";

        package Named;

        message Message {
            bool immediate = 1;
            oneof a {
                string a1 = 10;
                string a2 = 11;
            };
            oneof b {
                uint32 b1 = 20;
                uint32 b2 = 21;
            }

            enum Inner {
                value1 = 1;
                value2 = 2;
            }
        }
    "#])
    .unwrap();

    let mut handbuilt_context = Context::new();
    let package = handbuilt_context
        .insert_package(Package::new(Some("Named".to_string())))
        .unwrap();
    let mut message = MessageInfo::new("Message".to_string(), TypeParent::Package(package));

    let immediate = MessageField::new("immediate".to_string(), 1, ValueType::Bool);
    message.add_field(immediate).unwrap();

    // Here we add the oneof first and the fields refer to it.
    let oneof_first = Oneof::new("a".to_string());
    let oneof_ref = message.add_oneof(oneof_first).unwrap();

    let mut field_a1 = MessageField::new("a1".to_string(), 10, ValueType::String);
    field_a1.oneof = Some(oneof_ref);
    message.add_field(field_a1).unwrap();

    let mut field_a2 = MessageField::new("a2".to_string(), 11, ValueType::String);
    field_a2.oneof = Some(oneof_ref);
    message.add_field(field_a2).unwrap();

    // For b-fields add the fields first and then refer to them in the oneof.
    let field_b1 = MessageField::new("b1".to_string(), 20, ValueType::UInt32);
    message.add_field(field_b1).unwrap();
    let field_b2 = MessageField::new("b2".to_string(), 21, ValueType::UInt32);
    message.add_field(field_b2).unwrap();

    let mut oneof_b = Oneof::new("b".to_string());
    oneof_b.fields = vec![20, 21];
    message.add_oneof(oneof_b).unwrap();

    let message_ref = handbuilt_context.insert_message(message).unwrap();

    let mut inner_enum = EnumInfo::new("Inner".to_string(), TypeParent::Message(message_ref));
    inner_enum
        .add_field(EnumField::new("value1".to_string(), 1))
        .unwrap();
    inner_enum
        .add_field(EnumField::new("value2".to_string(), 2))
        .unwrap();

    handbuilt_context.insert_enum(inner_enum).unwrap();

    assert_eq!(parsed_context, handbuilt_context);
}

#[test]
fn iterate_fields() {
    let context = Context::parse(&[r#"
        syntax = "proto3";

        package Named;

        message Message {
            bool immediate = 1;
            oneof a {
                string a1 = 10;
                string a2 = 11;
            };
            oneof b {
                uint32 b1 = 20;
                uint32 b2 = 21;
            }

            enum Inner {
                value1 = 1;
                value2 = 2;
            }
        }
    "#])
    .unwrap();

    let message = context.get_message("Named.Message").unwrap();
    let mut fields = message.iter_fields();

    let immediate = fields.next().unwrap();
    assert_eq!(immediate.name, "immediate");
    assert!(immediate.oneof.is_none());

    let a1 = fields.next().unwrap();
    assert_eq!(a1.name, "a1");
    assert!(a1.oneof.is_some());
    assert_eq!(message.get_oneof(a1.oneof.unwrap()).unwrap().name, "a");
    let a2 = fields.next().unwrap();
    assert_eq!(a2.name, "a2");
    assert!(a2.oneof.is_some());
    assert_eq!(message.get_oneof(a2.oneof.unwrap()).unwrap().name, "a");

    let b1 = fields.next().unwrap();
    assert_eq!(b1.name, "b1");
    assert!(b1.oneof.is_some());
    assert_eq!(message.get_oneof(b1.oneof.unwrap()).unwrap().name, "b");
    let b2 = fields.next().unwrap();
    assert!(b2.oneof.is_some());
    assert_eq!(message.get_oneof(b2.oneof.unwrap()).unwrap().name, "b");
    assert_eq!(b2.name, "b2");

    assert_eq!(fields.next(), None);
}
