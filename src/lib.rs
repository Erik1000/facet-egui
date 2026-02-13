use std::marker::PhantomData;

use egui::{Response, Ui};
use facet::{Def, Facet, Shape, StructKind, Type, UserType};
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
                let inner_type = option_def.t().type_identifier;
                ui.horizontal(|ui| {
                    ui.label(format!("Option<{}>", inner_type));
                    // TODO: implement Option editing with is_some/get_value vtable
                    ui.label("(not yet implemented)")
                })
                .response
            }

            Def::Result(result_def) => {
                let ok_type = result_def.t().type_identifier;
                let err_type = result_def.e().type_identifier;
                ui.label(format!("Result<{}, {}>", ok_type, err_type))
            }

            Def::Map(map_def) => {
                let k_type = map_def.k().type_identifier;
                let v_type = map_def.v().type_identifier;
                ui.label(format!("Map<{}, {}> (not yet implemented)", k_type, v_type))
            }

            Def::Set(set_def) => {
                let t_type = set_def.t().type_identifier;
                ui.label(format!("Set<{}> (not yet implemented)", t_type))
            }

            Def::Pointer(_) => ui.label(format!("pointer: {}", shape.type_identifier)),

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
