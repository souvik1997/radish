#![feature(collections_range)]

#[macro_use]
extern crate nom;

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate diesel;
extern crate nix;
extern crate unicode_normalization;
extern crate unicode_segmentation;

mod shell;
use self::shell::Shell;

use std::process;

fn main() {
    process::exit(Shell::run() as i32);
}
