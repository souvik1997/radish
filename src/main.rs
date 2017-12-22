extern crate libc;
#[macro_use]
extern crate log;
#[macro_use]
extern crate nom;

mod shell;

fn main() {
    let mut s = shell::Shell::new();
    s.run_interactive();
}
