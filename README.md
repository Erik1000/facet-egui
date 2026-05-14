# [`facet-egui`](facet-egui/README.md) & [`facet-maybe-mut`](facet-maybe-mut/README.md)

[![Crates.io](https://img.shields.io/crates/v/facet-egui.svg)](https://crates.io/crates/facet-egui)
[![Docs.rs](https://docs.rs/facet-egui/badge.svg)](https://docs.rs/facet-egui)
[![CI](https://github.com/Erik1000/facet-egui/workflows/CI/badge.svg)](https://github.com/Erik1000/facet-egui/actions)
[![License](https://img.shields.io/crates/l/facet-egui.svg)](LICENSE-MIT)

Powerful reflection-based UI tools built on top of [`facet`](https://github.com/facet-rs/facet). This workspace contains two complementary crates for inspecting, editing, and working with data at runtime.

<p align="center">
  <a href="https://mhc-solutions.de/">
    <img src="https://www.mhc-solutions.de/files/content/logos/MHC-S-logo-farbig.svg" alt="MHC Solutions GmbH" />
  </a>
</p>

> Development of these crates is supported by [**MHC Solutions GmbH**](https://mhc-solutions.de), an engineering company focused on building, industrial, and energy automation whose support for open-source work helps make projects like this possible.

## `facet-egui`

An [egui](https://github.com/emilk/egui) inspector/editor widget that automatically generates property panels for any type implementing `facet::Facet`.

![FacetProbe Screenshot](facet-egui/examples/probe_gallery.png)

**Highlights:**
- Zero-boilerplate UI generation — just `#[derive(Facet)]`
- Recursive inspection and editing of structs, enums, lists, maps, and options
- Automatic lock handling for `Arc<RwLock<T>>` and `Arc<Mutex<T>>`
- Per-field customization with `#[facet(...)]` attributes
- Both editable and read-only modes

See [`facet-egui/README.md`](facet-egui/README.md) for full documentation and examples.

## `facet-maybe-mut`

A utility crate for working with `facet` types that may be read-only or mutable, with transparent lock handling for concurrent access patterns.

**Highlights:**
- Single code path for both read-only and mutable access
- Automatic lock acquisition for `Arc<RwLock<T>>` and `Arc<Mutex<T>>`
- Smart pointer dereferencing through `Arc`, `Rc`, `Box`, etc.
- Clean abstraction for conditional mutability

See [`facet-maybe-mut/README.md`](facet-maybe-mut/README.md) for full documentation and examples.

## How They Work Together

- **`facet-egui`** uses `facet-maybe-mut` internally to handle both editable and read-only modes, allowing a single code path to work with data whether it's directly accessible or behind locks.
- **`facet-maybe-mut`** provides the abstraction layer for transparent lock handling, which enables `facet-egui` to edit shared state in concurrent scenarios.

## Status

**Work in progress.** The API is not stable.

Due to `facet` not yet providing safe wrapper APIs for everything, both crates contain `unsafe` code which may not be sound.

## Architecture

Both crates are built on the `facet` reflection system:

- **`facet::Facet`** — Derive macro that adds type introspection to any struct or enum
- **`facet-reflect::Peek`** — Zero-copy immutable type reflection
- **`facet-reflect::Poke`** — Zero-copy mutable type reflection  
- **VTable dispatch** — Dynamic operations (push/pop/swap for lists, lock acquisition for smart pointers)
- **Custom attributes** — Per-field control via `#[facet(...)]` macros

See the individual crate READMEs for detailed feature documentation and API reference.

## Examples

- [probe_gallery](facet-egui/examples/probe_gallery.rs) — Complete showcase of `FacetProbe` in editable and readonly modes
- [user_type](facet-egui/examples/user_type.rs) — Creating custom user types
- [shared_string](facet-egui/examples/shared_string.rs) — Working with shared types

## Credits

- **`facet-egui`** is inspired by the [`egui-probe`](https://github.com/zakarumych/egui-probe) crate
- Big thanks to [**`facet`**](https://github.com/facet-rs/facet) for being an excellent Rust reflection library

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
