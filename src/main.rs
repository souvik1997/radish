#![feature(collections_range)]

#[macro_use]
extern crate nom;

#[macro_use]
extern crate bitflags;

extern crate nix;

extern crate unicode_normalization;
extern crate unicode_segmentation;

mod shell;

use std::process;

fn main() {
    let mut s = shell::Shell::new();
    process::exit(s.run_interactive() as i32);
}
