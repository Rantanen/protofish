#[test]
fn oneof() {
    use protofish::context::Context;

    // Hey at least we're ensuring this doesn't panic. :<
    Context::parse(&[r#"
      syntax = "proto3";
      message Message {
          oneof a {
            string a1 = 1;
            string a2 = 2;
            string a3 = 3;
          };
          oneof b {
            uint32 b1 = 4;
            uint32 b2 = 5;
            uint32 b3 = 6;
          }
      }
    "#])
    .unwrap();
}
