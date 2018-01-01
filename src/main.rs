extern crate libc;
#[macro_use]
extern crate log;
#[macro_use]
extern crate nom;

extern crate nix;

extern crate ketos;
#[macro_use]
extern crate ketos_derive;

mod shell;

use std::process;

fn main() {
    let mut s = shell::Shell::new();
    process::exit(s.run_interactive() as i32);
}
