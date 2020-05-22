use protofish;
use std::env;
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>>
{
    let data: Vec<String> = env::args()
        .skip(1)
        .map(|f| {
            let mut file = std::fs::File::open(f)?;
            let mut s = String::new();
            file.read_to_string(&mut s).unwrap();
            Ok(s)
        })
        .collect::<Result<_, std::io::Error>>()?;

    let data_ref: Vec<_> = data.iter().map(|s| s.as_str()).collect();
    let context = protofish::Context::parse(&data_ref)?;

    println!("{:#?}", context);

    Ok(())
}
