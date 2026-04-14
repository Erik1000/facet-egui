# facet-egui

An [egui](https://github.com/emilk/egui) inspector/editor widget for any type that implements [`Facet`](https://github.com/facet-rs/facet). Derive `Facet` on your types and get a full property editor with no additional boilerplate.

Built on top of [facet](https://github.com/facet-rs/facet)'s reflection system, this crate can inspect and mutate structs, enums, `Option`, `Vec`, maps, scalars, and nested combinations thereof — including types behind `Arc<RwLock<T>>` or `Arc<Mutex<T>>`.

## Workspace

This repository contains two crates:

### `facet-egui`

The main crate. Provides `FacetProbe`, a widget that renders an editable property panel for any `Facet` type.

Features:
- Recursive struct/enum/list/map/option inspection and editing
- Enum variant switching via combo box
- `Option<T>` toggle between `None` and `Some(T::default())`
- List manipulation (push/pop)
- Transparent traversal through smart pointers (`Arc`, `Rc`, `Box`, etc.)
- Automatic locking of `RwLock`/`Mutex` for shared types
- Attribute grammar for per-field control (`#[facet(egui::skip)]`, `#[facet(egui::readonly)]`, `#[facet(egui::rename("..."))]`)

### `facet-maybe-mut`

A utility crate that abstracts over shared and exclusive access to `Facet` types. It provides `MaybeMut`, an enum over `Peek` (read-only) and `Poke` (mutable) facet references, with the ability to transparently acquire locks through smart pointers.

Given a `&Arc<RwLock<T>>`, `MaybeMut` can walk through the `Arc`, acquire the `RwLock`, and hand back a mutable `Poke` to the inner `T` — hiding the locking details from the caller.

This is intentionally simple and not suitable for performance-critical code. It exists to make things like UI editors straightforward.

## Usage

```rust
use facet::Facet;
use facet_egui::FacetProbe;

#[derive(Debug, Facet, Default)]
pub struct Config {
    name: String,
    enabled: bool,
    count: u32,
}

// In your egui update loop:
fn show(ui: &mut egui::Ui, config: &mut Config) {
    FacetProbe::new(config).with_header("Config").show(ui);
}
```

It also works with shared types:

```rust
use std::sync::{Arc, RwLock};

let shared = Arc::new(RwLock::new(Config::default()));

// From any thread that has a clone of the Arc:
FacetProbe::new(&shared).show(ui);
```

## Attributes

Control field rendering with facet attributes:

```rust
#[derive(Facet)]
pub struct Player {
    name: String,

    #[facet(egui::skip)]
    internal_id: u64,

    #[facet(egui::readonly)]
    score: u32,

    #[facet(egui::rename("HP"))]
    health_points: f32,
}
```

## Status

Work in progress. The API is not stable. 

Due to `facet` not yet providing safe wrapper apis, `facet-egui` also contains
`unsafe` code which may not be sound.

# Credits

This crate is inspired by the great [`egui-probe`](https://github.com/zakarumych/egui-probe) crate which provides the same (and more) functionality with its own derive macro.

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