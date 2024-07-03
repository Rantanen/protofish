#[test]
fn repeated() {
    use bytes::BufMut;
    use protofish::{
        context::Context,
        decode::{FieldValue, MessageValue, PackedArray, Value},
    };

    let context = Context::parse(&[r#"
      syntax = "proto3";
      message Message {
          repeated string s = 1;
          repeated int32 small = 2;
          repeated int32 large = 3;
      }
    "#])
    .unwrap();

    let mut payload = bytes::BytesMut::new();

    payload.put_u8(1 << 3 | 2); // String tag.
    payload.put_u8(11);
    payload.put_slice(b"first value");

    payload.put_u8(1 << 3 | 2); // String tag.
    payload.put_u8(12);
    payload.put_slice(b"second value");

    payload.put_u8(2 << 3 | 2); // Packed integer array.
    payload.put_slice(b"\x06"); // Length
    payload.put_slice(b"\x01");
    payload.put_slice(b"\x80\x01");
    payload.put_slice(b"\x80\x80\x02");

    payload.put_u8(3 << 3 | 2); // Packed integer array.
    payload.put_slice(b"\x80\x01"); // Length
    payload.put_slice(&(b"\x01".repeat(128)));

    let msg = context.get_message("Message").unwrap();
    let value = msg.decode(&payload, &context);

    assert_eq!(
        value,
        MessageValue {
            msg_ref: msg.self_ref.clone(),
            garbage: None,
            fields: vec![
                FieldValue {
                    number: 1,
                    value: Value::String("first value".to_string()),
                },
                FieldValue {
                    number: 1,
                    value: Value::String("second value".to_string()),
                },
                FieldValue {
                    number: 2,
                    value: Value::Packed(PackedArray::Int32(vec![1, 1 << 7, 1 << 15])),
                },
                FieldValue {
                    number: 3,
                    value: Value::Packed(PackedArray::Int32(
                        std::iter::repeat(1).take(128).collect()
                    )),
                },
            ]
        }
    );

    let encoded = value.encode(&context);
    assert_eq!(payload, encoded);
}
