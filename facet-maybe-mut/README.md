# facet-maybe-mut

<p align="center">
  <a href="https://mhc-solutions.de/">
    <img src="https://www.mhc-solutions.de/files/content/logos/MHC-S-logo-farbig.svg" alt="MHC Solutions GmbH" />
  </a>
</p>

> Development of this crate is supported by [**MHC Solutions GmbH**](https://mhc-solutions.de), an engineering company focused on building, industrial, and energy automation whose support for open-source work helps make projects like this possible.

[![Crates.io](https://img.shields.io/crates/v/facet-maybe-mut.svg)](https://crates.io/crates/facet-maybe-mut)
[![Docs.rs](https://docs.rs/facet-maybe-mut/badge.svg)](https://docs.rs/facet-maybe-mut)
[![CI](https://github.com/Erik1000/facet-egui/workflows/CI/badge.svg)](https://github.com/Erik1000/facet-egui/actions)
[![License](https://img.shields.io/crates/l/facet-maybe-mut.svg)](LICENSE-MIT)

Utility crate for working with `facet` values that may be readonly (`Peek`) or mutable (`Poke`), including values behind lockable pointer types like `Arc<RwLock<T>>` and `Arc<Mutex<T>>`.

`MaybeMut` is useful when you want one code path that can inspect values and mutate when possible.

## Installation

```toml
[dependencies]
facet-maybe-mut = "0.0.2"
facet = "0.46"
facet-reflect = "0.46"
```

## Example: mutate through `Arc<RwLock<T>>`

```rust
# #[cfg(feature = "std")]
# fn main() {
use std::sync::{Arc, RwLock};

use facet::Facet;
use facet_maybe_mut::MaybeMut;
use facet_reflect::Peek;

#[derive(Debug, Facet)]
struct Being {
    age: u32,
}

let value = Arc::new(RwLock::new(Being { age: 20 }));

let mut guard = MaybeMut::Not(Peek::new(&value)).write().unwrap();
if let MaybeMut::Mut(poke) = &mut *guard {
    poke.get_mut::<Being>().unwrap().age += 1;
}
# }
#
# #[cfg(not(feature = "std"))]
# fn main() {}
```

## Example: read with automatic lock handling

```rust
# #[cfg(feature = "std")]
# fn main() {
use std::sync::{Arc, RwLock};

use facet::Facet;
use facet_maybe_mut::MaybeMut;
use facet_reflect::Peek;

#[derive(Debug, Facet)]
struct Being {
    age: u32,
}

let value = Arc::new(RwLock::new(Being { age: 20 }));

let guard = MaybeMut::Not(Peek::new(&value)).read().unwrap();
let age = guard.as_peek().get::<Being>().unwrap().age;
assert_eq!(age, 20);
# }
#
# #[cfg(not(feature = "std"))]
# fn main() {}
```

## Notes

This crate intentionally trades ergonomics for strict control over locking details. In hot paths, prefer more explicit access patterns.

Also, this crate contains `unsafe` code which may not be sound.


## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0)>
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
