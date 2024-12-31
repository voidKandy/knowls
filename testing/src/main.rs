use std::io::{self, Read};

// @_ hello
fn main() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
    println!("{raw}");
}

// %s
struct ToBePushed {
    x: u32,
}
impl ToBePushed {
    fn x(&self) -> &u32 {
        &self.x
    }
}
