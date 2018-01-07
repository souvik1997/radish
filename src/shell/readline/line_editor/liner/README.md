# liner
A Rust library offering readline-like functionality.

[CONTRIBUTING.md](/CONTRIBUTING.md)

[![crates.io](https://meritbadge.herokuapp.com/liner)](https://crates.io/crates/liner)
[![Build Status](https://travis-ci.org/MovingtoMars/liner.svg)](https://travis-ci.org/MovingtoMars/liner)
[![Docs](https://docs.rs/liner/badge.svg)](https://docs.rs/liner/)

## Featues
- [x] Autosuggestions
- [x] Emacs and Vi keybindings
- [x] Multi-line editing
- [x] History
- [x] (Incomplete) basic and filename completions
- [ ] Reverse search
- [ ] Remappable keybindings

## Basic Usage
In `Cargo.toml`:
```toml
[dependencies]
liner = "0.4.3"
...
```

In `src/main.rs`:

```rust
extern crate liner;

use liner::Context;

fn main() {
    let mut con = Context::new();

    loop {
        let res = con.read_line("[prompt]$ ", &mut |_| {}).unwrap();

        if res.is_empty() {
            break;
        }

        con.history.push(res.into());
    }
}
```

**See src/main.rs for a more sophisticated example.**

## License
MIT licensed. See the `LICENSE` file.
