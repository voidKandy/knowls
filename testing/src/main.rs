use std::io::{self, Read};

fn main() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read io");
    println!("{raw}");
}

struct ToBePushed {
    x: u32,
}
impl ToBePushed {
    fn x(&self) -> &u32 {
        &self.x
    }
}
