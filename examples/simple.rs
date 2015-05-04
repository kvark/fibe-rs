extern crate fibe;

use fibe::*;

fn test(wait: Wait) {
    let mut front = Frontend::new();
    let ha = task(move |_| {print!("Hello, ")}).start(&mut front);
    task(move |_| {println!("World!")}).after(ha).start(&mut front);
    front.die(wait);
}

fn main() {
    test(Wait::None);
    test(Wait::Active);
    test(Wait::Pending);
}
