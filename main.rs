use derive_debug::CustomDebug;

#[derive(CustomDebug)]
pub struct Field {
    name: &'static str,
    bitmask: u8,
}

fn main() {
    let f = Field {
        name: "F",
        bitmask: 0b00011100,
    };

    let debug = format!("{:?}", f);

    assert!(debug.starts_with(r#"Field { name: "F","#));
}
