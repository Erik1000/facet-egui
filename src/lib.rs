use std::marker::PhantomData;

use egui::{Response, Ui};
#[cfg(feature = "dyn_trait")]
use facet::Shape;
use facet::{Def, Facet, StructKind, Type, UserType};
use facet_reflect::Poke;

// ============================================================================
// dyn_trait feature: FacetShape trait for trait object reflection
// ============================================================================

/// A dyn-compatible trait that provides the [`Shape`] of the implementing type.
///
/// This enables creating a [`Poke`] from a trait object (`&mut dyn FacetShape`)
/// when you don't know the concrete type at compile time.
///
/// # Safety Contract
///
/// Implementors **must** return the correct `Shape` for their concrete type.
/// The blanket implementation for `T: Facet` guarantees this automatically.
/// Manual implementations that return an incorrect `Shape` will cause undefined behavior.
///
/// # Example
///
/// ```ignore
/// use facet::Facet;
/// use facet_egui::{FacetShape, poke_from_mut};
///
/// #[derive(Facet)]
/// struct MyData { value: i32 }
///
/// let mut data: Box<dyn FacetShape> = Box::new(MyData { value: 42 });
/// let poke = poke_from_mut(&mut *data);
/// ```
#[cfg(feature = "dyn_trait")]
pub trait FacetShape {
    /// Returns the static [`Shape`] describing this type.
    fn shape(&self) -> &'static Shape;
}

#[cfg(feature = "dyn_trait")]
impl<T: Facet<'static>> FacetShape for T {
    fn shape(&self) -> &'static Shape {
        T::SHAPE
    }
}

/// Creates a [`Poke`] from a mutable reference to a trait object implementing [`FacetShape`].
///
/// This allows runtime reflection over `Box<dyn FacetShape>` or `&mut dyn FacetShape`
/// when the concrete type is not known at compile time.
///
/// # Safety
///
/// This function is safe **if and only if** the `FacetShape` implementation
/// returns the correct `Shape` for the concrete type. The blanket implementation
/// guarantees this for all `T: Facet<'static>`.
///
/// # Example
///
/// ```ignore
/// let mut items: Vec<Box<dyn FacetShape>> = vec![
///     Box::new(42i32),
///     Box::new(String::from("hello")),
/// ];
///
/// for item in &mut items {
///     let poke = poke_from_mut(&mut **item);
///     // Use poke for reflection...
/// }
/// ```
#[cfg(feature = "dyn_trait")]
pub fn poke_from_mut<'a>(obj: &'a mut dyn FacetShape) -> Poke<'a, 'static> {
    use facet::PtrMut;

    let shape = obj.shape();

    // Extract the data pointer from the fat pointer
    let fat_ptr: *mut dyn FacetShape = obj;
    let data_ptr: *mut u8 = fat_ptr as *mut u8;

    // SAFETY:
    // - data_ptr points to a valid, initialized value (from the mutable reference)
    // - shape is guaranteed by FacetShape to match the concrete type
    //   (enforced by blanket impl for T: Facet)
    // - lifetime 'a is valid for the duration of the borrow
    unsafe { Poke::from_raw_parts(PtrMut::new(data_ptr), shape) }
}

// ============================================================================
// Map UI helpers - generated via macro for common HashMap<String, V> types
// ============================================================================

macro_rules! impl_string_map_ui {
    ($fn_name:ident, $value_type:ty, $default_value:expr, $editor:expr) => {
        fn $fn_name(map: &mut std::collections::HashMap<String, $value_type>, ui: &mut Ui) {
            let mut keys_to_remove = Vec::new();
            let mut entries: Vec<_> = map.iter_mut().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));

            for (key, value) in entries {
                ui.horizontal(|ui| {
                    ui.label(format!("[{}]", key));
                    #[allow(clippy::redundant_closure_call)]
                    ($editor)(value, ui);
                    if ui.small_button("🗑").clicked() {
                        keys_to_remove.push(key.clone());
                    }
                });
            }

            for key in keys_to_remove {
                map.remove(&key);
            }

            if map.is_empty() {
                ui.label("(empty)");
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("+ Add entry").clicked() {
                    let mut i = 0;
                    loop {
                        let new_key = format!("key_{}", i);
                        if !map.contains_key(&new_key) {
                            map.insert(new_key, $default_value);
                            break;
                        }
                        i += 1;
                    }
                }
            });
        }
    };
}

impl_string_map_ui!(
    render_string_string_map_ui,
    String,
    String::new(),
    |v: &mut String, ui: &mut Ui| {
        ui.text_edit_singleline(v);
    }
);
impl_string_map_ui!(
    render_string_i32_map_ui,
    i32,
    0i32,
    |v: &mut i32, ui: &mut Ui| {
        ui.add(egui::DragValue::new(v));
    }
);
impl_string_map_ui!(
    render_string_i64_map_ui,
    i64,
    0i64,
    |v: &mut i64, ui: &mut Ui| {
        ui.add(egui::DragValue::new(v));
    }
);
impl_string_map_ui!(
    render_string_u32_map_ui,
    u32,
    0u32,
    |v: &mut u32, ui: &mut Ui| {
        ui.add(egui::DragValue::new(v));
    }
);
impl_string_map_ui!(
    render_string_u64_map_ui,
    u64,
    0u64,
    |v: &mut u64, ui: &mut Ui| {
        ui.add(egui::DragValue::new(v));
    }
);
impl_string_map_ui!(
    render_string_f32_map_ui,
    f32,
    0.0f32,
    |v: &mut f32, ui: &mut Ui| {
        ui.add(egui::DragValue::new(v));
    }
);
impl_string_map_ui!(
    render_string_f64_map_ui,
    f64,
    0.0f64,
    |v: &mut f64, ui: &mut Ui| {
        ui.add(egui::DragValue::new(v));
    }
);
impl_string_map_ui!(
    render_string_bool_map_ui,
    bool,
    false,
    |v: &mut bool, ui: &mut Ui| {
        ui.checkbox(v, "");
    }
);

pub struct FacetProbe<'a, 'f, T> {
    phantom: PhantomData<&'f ()>,
    inner: &'a mut T,
}

impl<'a, 'f, T> FacetProbe<'a, 'f, T>
where
    T: Facet<'f>,
    'a: 'f,
{
    pub fn new(inner: &'a mut T) -> FacetProbe<'a, 'f, T> {
        Self {
            inner,
            phantom: PhantomData,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) -> Response {
        let poke = Poke::new(self.inner);
        Self::poke_ui(poke, ui)
    }

    /// Renders UI for a [`Poke`] directly.
    ///
    /// This is useful when working with trait objects via [`poke_from_mut`],
    /// or when you already have a `Poke` from other reflection operations.
    pub fn poke_ui(mut poke: Poke<'_, 'f>, ui: &mut Ui) -> Response {
        let shape = poke.shape();

        // First, try to match on Def for semantic handling
        match shape.def {
            Def::Scalar => {
                // Try common scalar types
                if let Ok(v) = poke.get_mut::<bool>() {
                    return ui.checkbox(v, "");
                }
                if let Ok(v) = poke.get_mut::<String>() {
                    return ui.text_edit_singleline(v);
                }
                if let Ok(c) = poke.get_mut::<char>() {
                    let mut s = c.to_string();
                    let resp = ui.text_edit_singleline(&mut s);
                    if let Some(new_char) = s.chars().next() {
                        *c = new_char;
                    }
                    return resp;
                }
                // Floats
                if let Ok(v) = poke.get_mut::<f32>() {
                    return ui.add(egui::DragValue::new(v));
                }
                if let Ok(v) = poke.get_mut::<f64>() {
                    return ui.add(egui::DragValue::new(v));
                }
                // Signed integers
                if let Ok(v) = poke.get_mut::<i8>() {
                    return ui.add(egui::DragValue::new(v));
                }
                if let Ok(v) = poke.get_mut::<i16>() {
                    return ui.add(egui::DragValue::new(v));
                }
                if let Ok(v) = poke.get_mut::<i32>() {
                    return ui.add(egui::DragValue::new(v));
                }
                if let Ok(v) = poke.get_mut::<i64>() {
                    return ui.add(egui::DragValue::new(v));
                }
                if let Ok(v) = poke.get::<i128>() {
                    return ui.label(format!("{}", v));
                }
                if let Ok(v) = poke.get_mut::<isize>() {
                    return ui.add(egui::DragValue::new(v));
                }
                // Unsigned integers
                if let Ok(v) = poke.get_mut::<u8>() {
                    return ui.add(egui::DragValue::new(v));
                }
                if let Ok(v) = poke.get_mut::<u16>() {
                    return ui.add(egui::DragValue::new(v));
                }
                if let Ok(v) = poke.get_mut::<u32>() {
                    return ui.add(egui::DragValue::new(v));
                }
                if let Ok(v) = poke.get_mut::<u64>() {
                    return ui.add(egui::DragValue::new(v));
                }
                if let Ok(v) = poke.get::<u128>() {
                    return ui.label(format!("{}", v));
                }
                if let Ok(v) = poke.get_mut::<usize>() {
                    return ui.add(egui::DragValue::new(v));
                }
                // Unknown scalar
                ui.label(format!("scalar: {}", shape.type_identifier))
            }

            Def::List(list_def) => {
                let type_name = shape.type_identifier;
                let elem_type = list_def.t().type_identifier;
                egui::CollapsingHeader::new(format!("{}<{}>", type_name, elem_type))
                    .default_open(false)
                    .show(ui, |ui| {
                        if let Ok(mut list) = poke.into_list() {
                            let len = list.len();
                            for i in 0..len {
                                if let Some(elem) = list.get_mut(i) {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("[{}]", i));
                                        Self::poke_ui(elem, ui);
                                    });
                                }
                            }
                        }
                    })
                    .header_response
            }

            Def::Array(array_def) => {
                let type_name = shape.type_identifier;
                egui::CollapsingHeader::new(format!("{} [{}]", type_name, array_def.n))
                    .default_open(false)
                    .show(ui, |ui| {
                        if let Ok(mut list) = poke.into_list() {
                            let len = list.len();
                            for i in 0..len {
                                if let Some(elem) = list.get_mut(i) {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("[{}]", i));
                                        Self::poke_ui(elem, ui);
                                    });
                                }
                            }
                        }
                    })
                    .header_response
            }

            Def::Slice(_) => {
                let type_name = shape.type_identifier;
                egui::CollapsingHeader::new(type_name)
                    .default_open(false)
                    .show(ui, |ui| {
                        if let Ok(mut list) = poke.into_list() {
                            let len = list.len();
                            for i in 0..len {
                                if let Some(elem) = list.get_mut(i) {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("[{}]", i));
                                        Self::poke_ui(elem, ui);
                                    });
                                }
                            }
                        }
                    })
                    .header_response
            }

            Def::Option(option_def) => {
                let inner_shape = option_def.t();
                let inner_type = inner_shape.type_identifier;

                // Use as_peek() to get a Peek, then into_option()
                let peek = poke.as_peek();
                if let Ok(peek_option) = peek.into_option() {
                    if let Some(inner_peek) = peek_option.value() {
                        // It's Some - show the inner value (read-only for now)
                        ui.horizontal(|ui| {
                            ui.label(format!("Option<{}>: Some", inner_type));
                        });
                        ui.indent("option_inner", |ui| {
                            ui.label(format!("{}", inner_peek));
                        });
                    } else {
                        // It's None
                        ui.horizontal(|ui| {
                            ui.label(format!("Option<{}>: None", inner_type));
                        });
                    }
                } else {
                    ui.label(format!("Option<{}> (access error)", inner_type));
                }

                ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
            }

            Def::Result(result_def) => {
                let ok_shape = result_def.t();
                let err_shape = result_def.e();
                let ok_type = ok_shape.type_identifier;
                let err_type = err_shape.type_identifier;

                // Use as_peek() to get a Peek, then into_result()
                let peek = poke.as_peek();
                if let Ok(peek_result) = peek.into_result() {
                    if let Some(ok_peek) = peek_result.ok() {
                        // It's Ok (read-only display)
                        ui.horizontal(|ui| {
                            ui.label(format!("Result<{}, {}>: Ok", ok_type, err_type));
                        });
                        ui.indent("result_ok", |ui| {
                            ui.label(format!("{}", ok_peek));
                        });
                    } else if let Some(err_peek) = peek_result.err() {
                        // It's Err (read-only display)
                        ui.horizontal(|ui| {
                            ui.label(format!("Result<{}, {}>: Err", ok_type, err_type));
                        });
                        ui.indent("result_err", |ui| {
                            ui.label(format!("{}", err_peek));
                        });
                    } else {
                        ui.label(format!("Result<{}, {}> (invalid state)", ok_type, err_type));
                    }
                } else {
                    ui.label(format!("Result<{}, {}> (access error)", ok_type, err_type));
                }

                ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
            }

            Def::Map(map_def) => {
                use std::collections::HashMap;

                let k_shape = map_def.k();
                let v_shape = map_def.v();
                let k_type = k_shape.type_identifier;
                let v_type = v_shape.type_identifier;

                egui::CollapsingHeader::new(format!("Map<{}, {}>", k_type, v_type))
                    .default_open(false)
                    .show(ui, |ui| {
                        // Try common HashMap<String, V> types with full editing support
                        let handled = if let Ok(map) = poke.get_mut::<HashMap<String, String>>() {
                            render_string_string_map_ui(map, ui);
                            true
                        } else if let Ok(map) = poke.get_mut::<HashMap<String, i32>>() {
                            render_string_i32_map_ui(map, ui);
                            true
                        } else if let Ok(map) = poke.get_mut::<HashMap<String, i64>>() {
                            render_string_i64_map_ui(map, ui);
                            true
                        } else if let Ok(map) = poke.get_mut::<HashMap<String, u32>>() {
                            render_string_u32_map_ui(map, ui);
                            true
                        } else if let Ok(map) = poke.get_mut::<HashMap<String, u64>>() {
                            render_string_u64_map_ui(map, ui);
                            true
                        } else if let Ok(map) = poke.get_mut::<HashMap<String, f32>>() {
                            render_string_f32_map_ui(map, ui);
                            true
                        } else if let Ok(map) = poke.get_mut::<HashMap<String, f64>>() {
                            render_string_f64_map_ui(map, ui);
                            true
                        } else if let Ok(map) = poke.get_mut::<HashMap<String, bool>>() {
                            render_string_bool_map_ui(map, ui);
                            true
                        } else {
                            false
                        };

                        if !handled {
                            // Fall back to read-only display using Peek
                            let peek = poke.as_peek();
                            if let Ok(peek_map) = peek.into_map() {
                                let mut count = 0;
                                for (key, value) in peek_map.iter() {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("[{}]", key));
                                        ui.label(format!("{}", value));
                                    });
                                    count += 1;
                                }
                                if count == 0 {
                                    ui.label("(empty)");
                                }
                                ui.label("(read-only: type not directly editable)");
                            }
                        }
                    })
                    .header_response
            }

            Def::Set(set_def) => {
                let t_type = set_def.t().type_identifier;

                egui::CollapsingHeader::new(format!("Set<{}>", t_type))
                    .default_open(false)
                    .show(ui, |ui| {
                        // Use Peek for read-only iteration
                        let peek = poke.as_peek();
                        if let Ok(peek_set) = peek.into_set() {
                            let mut count = 0;
                            for value in peek_set.iter() {
                                ui.label(format!("{}", value));
                                count += 1;
                            }
                            if count == 0 {
                                ui.label("(empty)");
                            }
                        }
                    })
                    .header_response
            }

            Def::Pointer(pointer_def) => {
                let type_name = shape.type_identifier;

                // pointee is Option<&Shape>, so handle it appropriately
                if let Some(pointee_shape) = pointer_def.pointee {
                    egui::CollapsingHeader::new(format!(
                        "{}<{}>",
                        type_name, pointee_shape.type_identifier
                    ))
                    .id_salt(ui.next_auto_id())
                    .default_open(true)
                    .show(ui, |ui| {
                        // Use Peek for read-only view of pointer content
                        let peek = poke.as_peek();
                        if let Ok(peek_ptr) = peek.into_pointer() {
                            // Use borrow_inner to access the pointed-to value
                            if let Some(inner_peek) = peek_ptr.borrow_inner() {
                                ui.label(format!("{}", inner_peek));
                            } else {
                                ui.label(format!("-> {}", pointee_shape.type_identifier));
                            }
                        } else {
                            ui.label("(inaccessible)");
                        }
                    })
                    .header_response
                } else {
                    // Opaque pointer with unknown pointee
                    ui.label(format!("{} (opaque pointer)", type_name))
                }
            }

            Def::NdArray(_) => ui.label(format!("ndarray: {}", shape.type_identifier)),

            Def::DynamicValue(_) => ui.label(format!("dynamic: {}", shape.type_identifier)),

            // For Undefined def, fall back to Type-based dispatch
            Def::Undefined => Self::poke_ui_by_type(poke, ui),

            // Non-exhaustive catch-all
            _ => ui.label(format!("unknown def: {}", shape.type_identifier)),
        }
    }

    /// Fallback for types without a Def (user structs, enums, etc.)
    fn poke_ui_by_type(poke: Poke<'_, 'f>, ui: &mut Ui) -> Response {
        let shape = poke.shape();

        match shape.ty {
            Type::User(user_type) => match user_type {
                UserType::Struct(struct_type) => {
                    let type_name = shape.type_identifier;
                    egui::CollapsingHeader::new(type_name)
                        .id_salt(ui.next_auto_id())
                        .default_open(true)
                        .show(ui, |ui| {
                            if let Ok(mut poke_struct) = poke.into_struct() {
                                for (i, field) in struct_type.fields.iter().enumerate() {
                                    let name = if field.name.is_empty() {
                                        format!("{}", i)
                                    } else {
                                        field.name.to_string()
                                    };
                                    if let Ok(field_poke) = poke_struct.field(i) {
                                        ui.horizontal(|ui| {
                                            ui.label(name);
                                            Self::poke_ui(field_poke, ui);
                                        });
                                    }
                                }
                            }
                        })
                        .header_response
                }
                UserType::Enum(enum_type) => {
                    let type_name = shape.type_identifier;
                    if let Ok(mut poke_enum) = poke.into_enum() {
                        let current_variant = poke_enum.variant_name_active().unwrap_or("?");

                        egui::CollapsingHeader::new(format!("{}::{}", type_name, current_variant))
                            .id_salt(ui.next_auto_id())
                            .default_open(true)
                            .show(ui, |ui| {
                                // Show variant selector (read-only for now)
                                egui::ComboBox::from_label("variant")
                                    .selected_text(current_variant)
                                    .show_ui(ui, |ui| {
                                        for variant in enum_type.variants {
                                            let _ = ui.selectable_label(
                                                variant.name == current_variant,
                                                variant.name,
                                            );
                                        }
                                    });

                                // Show fields of current variant
                                if let Ok(variant) = poke_enum.active_variant() {
                                    match variant.data.kind {
                                        StructKind::Unit => {}
                                        StructKind::TupleStruct | StructKind::Tuple => {
                                            let field_count = variant.data.fields.len();
                                            for i in 0..field_count {
                                                if let Ok(Some(field_poke)) = poke_enum.field(i) {
                                                    ui.horizontal(|ui| {
                                                        ui.label(format!(".{}", i));
                                                        Self::poke_ui(field_poke, ui);
                                                    });
                                                }
                                            }
                                        }
                                        StructKind::Struct => {
                                            for (i, field) in variant.data.fields.iter().enumerate()
                                            {
                                                let name = if field.name.is_empty() {
                                                    format!("{}", i)
                                                } else {
                                                    field.name.to_string()
                                                };
                                                if let Ok(Some(field_poke)) = poke_enum.field(i) {
                                                    ui.horizontal(|ui| {
                                                        ui.label(name);
                                                        Self::poke_ui(field_poke, ui);
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            })
                            .header_response
                    } else {
                        ui.label(format!("enum {} (error)", type_name))
                    }
                }
                UserType::Union(union_type) => {
                    ui.label(format!("union ({} fields)", union_type.fields.len()))
                }
                UserType::Opaque => ui.label(format!("opaque: {}", shape.type_identifier)),
            },
            _ => ui.label(format!("unhandled type: {}", shape.type_identifier)),
        }
    }
}
