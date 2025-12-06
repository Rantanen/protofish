#[test]
fn encode_message()
{
    use protofish::{
        context::Context,
        decode::{FieldValue, MessageValue, Value},
    };

    let context = Context::parse(&[r#"
      syntax = "proto3";
      message Message {
          string s = 1;
          int32 small = 2;
          int64 large = 3;
          sint32 signed = 4;
          fixed64 fixed = 5;
          double dbl = 6;
          bool b = 7;
          Message child = 10;
      }
    "#])
    .unwrap();

    let msg = context.get_message("Message").unwrap();

    let original = MessageValue {
        msg_ref: msg.self_ref.clone(),
        garbage: None,
        fields: vec![
            FieldValue {
                number: 1,
                value: Value::String("parent".to_string()),
            },
            FieldValue {
                number: 2,
                value: Value::Int32(123),
            },
            FieldValue {
                number: 3,
                value: Value::Int64(12356),
            },
            FieldValue {
                number: 4,
                value: Value::SInt32(-123),
            },
            FieldValue {
                number: 5,
                value: Value::Fixed64(12356),
            },
            FieldValue {
                number: 6,
                value: Value::Double(1.2345),
            },
            FieldValue {
                number: 7,
                value: Value::Bool(true),
            },
            FieldValue {
                number: 10,
                value: Value::Message(Box::new(MessageValue {
                    msg_ref: msg.self_ref.clone(),
                    garbage: None,
                    fields: vec![FieldValue {
                        number: 1,
                        value: Value::String("child".to_string()),
                    }],
                })),
            },
        ],
    };

    let expected = original.encode(&context);
    let decoded = msg.decode(&expected, &context);
    let actual = decoded.encode(&context);

    assert_eq!(original, decoded);
    assert_eq!(expected, actual);
}
