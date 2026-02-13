# Note 

This is vibe coded by claude opus and just a PoC

# facet-egui

Automatic [egui](https://github.com/emilk/egui) UI generation for any type implementing [Facet](https://github.com/facet-rs/facet).

## Overview

`facet-egui` leverages Facet's reflection capabilities to automatically generate editable UI widgets for your Rust types. No manual widget code required - just derive `Facet` and call `FacetProbe::ui()`.

## Features

- **Automatic UI generation** for structs, enums, and primitive types
- **Nested type support** - complex hierarchies render as collapsible sections
- **Container support** - Vec, arrays, HashMap, HashSet, Option, Result
- **Smart pointer support** - works with Arc, RwLock, Box patterns
- **Trait object reflection** (optional) - edit `Box<dyn Trait>` collections

## Installation

```toml
[dependencies]
facet-egui = "0.1"
facet = "0.43"
```

For trait object support:

```toml
[dependencies]
facet-egui = { version = "0.1", features = ["dyn_trait"] }
```

## Quick Start

```rust
use facet::Facet;
use facet_egui::FacetProbe;

#[derive(Facet)]
struct Player {
    name: String,
    health: i32,
    position: (f32, f32),
}

// In your egui update loop:
fn update(&mut self, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        FacetProbe::new(&mut self.player).ui(ui);
    });
}
```

## Trait Object Reflection (`dyn_trait` feature)

When you have heterogeneous collections of different Facet types, use the `dyn_trait` feature:

```rust
use facet::Facet;
use facet_egui::{FacetShape, FacetProbe, poke_from_mut};

#[derive(Facet)]
struct Enemy { health: i32 }

#[derive(Facet)]  
struct Item { name: String }

// FacetShape is automatically implemented for all Facet types
let mut entities: Vec<Box<dyn FacetShape>> = vec![
    Box::new(Enemy { health: 100 }),
    Box::new(Item { name: "Sword".into() }),
];

// Edit each item regardless of concrete type
for entity in &mut entities {
    let poke = poke_from_mut(&mut **entity);
    FacetProbe::<()>::poke_ui(poke, ui);
}
```

## Examples

Run the examples to see `facet-egui` in action:

```bash
# Basic struct/enum editing
cargo run --example basic

# Complex container types (Vec, HashMap, etc.)
cargo run --example containers

# Smart pointers (Arc, RwLock, Box)
cargo run --example smart_pointers

# Trait object collections (requires dyn_trait feature)
cargo run --example dyn_trait --features dyn_trait
```

## Supported Types

| Category | Types |
|----------|-------|
| Scalars | `bool`, `char`, `String`, `i8`-`i128`, `u8`-`u128`, `f32`, `f64` |
| Containers | `Vec<T>`, `[T; N]`, `&[T]` |
| Maps | `HashMap<K, V>` (display only) |
| Sets | `HashSet<T>` (display only) |
| Options | `Option<T>` |
| Results | `Result<T, E>` |
| User Types | Structs, Enums (with variants), Tuples |

## API Reference

### `FacetProbe<T>`

The main entry point for rendering Facet types:

```rust
impl<T: Facet> FacetProbe<'_, '_, T> {
    /// Create a new probe wrapping a mutable reference
    pub fn new(inner: &mut T) -> Self;
    
    /// Render the UI and return the egui Response
    pub fn ui(&mut self, ui: &mut Ui) -> Response;
    
    /// Render a Poke directly (useful for trait objects)
    pub fn poke_ui(poke: Poke, ui: &mut Ui) -> Response;
}
```

### `FacetShape` (requires `dyn_trait` feature)

A dyn-compatible trait for runtime shape access:

```rust
pub trait FacetShape {
    fn shape(&self) -> &'static Shape;
}

// Blanket impl provided for all T: Facet<'static>
```

### `poke_from_mut()` (requires `dyn_trait` feature)

Convert a trait object to a `Poke`:

```rust
pub fn poke_from_mut<'a>(obj: &'a mut dyn FacetShape) -> Poke<'a, 'static>;
```

## License

MIT OR Apache-2.0