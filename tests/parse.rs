#[test]
fn parse() {
    use protofish::context::{
        Context, MessageField, MessageInfo, Multiplicity, Package, TypeParent, ValueType,
    };

    let context = Context::parse(&[r#"
      syntax = "proto3";
      message Message {
          string s = 1;
          repeated bytes b = 2;
          optional int64 large = 3;
          repeated sint32 signed = 4;
          Message child = 10;
      }
    "#])
    .unwrap();

    let mut expected = Context::new();
    let package = expected.insert_package(Package::new(None)).unwrap();
    let mut message = MessageInfo::new("Message".to_string(), TypeParent::Package(package));

    message
        .add_field(MessageField::new("s".to_string(), 1, ValueType::String))
        .unwrap();

    let mut b_field = MessageField::new("b".to_string(), 2, ValueType::Bytes);
    b_field.multiplicity = Multiplicity::Repeated;

    let mut large_field = MessageField::new("large".to_string(), 3, ValueType::Int64);
    large_field.multiplicity = Multiplicity::Optional;

    let mut signed_field = MessageField::new("signed".to_string(), 4, ValueType::SInt32);
    signed_field.multiplicity = Multiplicity::RepeatedPacked;

    let child_field = MessageField::new(
        "child".to_string(),
        10,
        ValueType::Message(message.self_ref),
    );

    message.add_field(b_field).unwrap();
    message.add_field(large_field).unwrap();
    message.add_field(signed_field).unwrap();
    message.add_field(child_field).unwrap();

    expected.insert_message(message).unwrap();

    assert_eq!(expected, context);
}
