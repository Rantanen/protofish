#[test]
fn maptype() {
    use protofish::context::Context;

    // Hey at least we're ensuring this doesn't panic. :<
    Context::parse(&[r#"
      syntax = "proto3";
      message Message {
          message mydata {
            uint32 b1 = 1;
            map<int32, string> m1 = 2;
            map<int32, string> m2 = 3;
            map<int32, mydata> m3 = 4;
          }
      }
    "#])
    .unwrap();
}
