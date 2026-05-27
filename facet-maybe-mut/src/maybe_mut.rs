use derive_more::{Deref, DerefMut, From};
use facet::{Def, PointerFlags, PtrConst, PtrMut, ReadLockResult, Shape, WriteLockResult};
use facet_reflect::{Peek, Poke};

/// Some reference to a type that implements [`Facet`](facet::Facet) that may be
/// `mut` or not.
#[derive(From)]
#[repr(C)]
pub enum MaybeMut<'mem, 'facet> {
    Not(Peek<'mem, 'facet>),
    Mut(Poke<'mem, 'facet>),
}

impl<'mem, 'facet> MaybeMut<'mem, 'facet> {
    /// Returns a readonly/immutable version of the inner type
    pub fn as_peek(&'mem self) -> Peek<'mem, 'facet> {
        match self {
            Self::Not(peek) => *peek,
            Self::Mut(poke) => poke.as_peek(),
        }
    }

    pub fn into_peek(self) -> Peek<'mem, 'facet> {
        match self {
            MaybeMut::Not(n) => n,
            MaybeMut::Mut(m) => m.into_peek(),
        }
    }

    /// Returns the [`Shape`] of the underlying type
    ///
    /// The [`Shape`] is the same for [`Mut`](Self::Mut) and [`Not`](Self::Not)
    pub fn shape(&self) -> &'static Shape {
        self.as_peek().shape()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct MakeLockError<'mem, 'facet> {
    pub unchanged: Peek<'mem, 'facet>,
    pub kind: MakeLockErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub enum MakeLockErrorKind {
    /// The underlying type is not a type that we can lock from a `&T` to `&mut T` (but it is more complicated...)
    #[error("type cannot be locked")]
    NotLockable,
    /// The underlying type could be locked but the provided lock method in the
    /// vtable returned an error.
    #[error("locking of type failed")]
    LockFailure,
    /// A weak pointer where the upgrade function returned None.
    ///
    /// There exist no strong references (no instances of an `Arc`)
    #[error("could not upgrade weak pointer, no strong references exist")]
    NotUpgradable,
}

/// Depending on whether this is a read or write lock, `P` will be either
/// [`PtrConst`] or [`PtrMut`](facet::PtrMut). This enum makes `P` dynamic
#[derive(From)]
pub(crate) enum LockGuardType {
    Write(WriteLockResult),
    Read(ReadLockResult),
    /// A Weak that has been upgraded to an Arc or Rc
    /// The downgrade is handled directly in the [`Drop`] implementation of this
    /// [`LockGuardType`]
    Upgrade {
        /// The [`Shape`] of the strong pointer that can be used later to
        /// downgrade again
        strong_shape: &'static Shape,
        /// The pointer to the allocated Arc or Rc
        allocation: PtrMut,
    },
}

impl LockGuardType {
    /// Safety
    ///
    /// For an Weak guard, this calls the `BorrowFn` of the (currently existing)
    /// strong shape to obtain the data pointer.
    /// This is the raw pointer returned from the lock which is already
    /// available via [`Guard`]. Creating a new [`Peek`] or [`Poke`] from this
    /// [`PtrConst`] is UB.
    pub fn data_const(&self) -> PtrConst {
        match self {
            Self::Write(w) => w.data_const(),
            Self::Read(r) => *r.data(),
            Self::Upgrade {
                strong_shape,
                allocation,
            } => {
                let borrow_fn = strong_shape
                    .def
                    .into_pointer()
                    .expect("only pointer types get this lock type")
                    .vtable
                    .borrow_fn
                    .expect("all strong pointers have a borrow function");
                // SAFETY: allocation is the pointer of the strong type (Arc or Rc)
                unsafe { borrow_fn(allocation.as_const()) }
            }
        }
    }
}

impl Drop for LockGuardType {
    fn drop(&mut self) {
        if let Self::Upgrade {
            strong_shape,
            allocation,
        } = self
        {
            // dropping the strong pointer automatically decreases reference
            // count
            // SAFETY: we cant just deallocate but need to run the actual drop
            // implementation as well since the drop impl of the strong pointer decreases strong pointer count
            unsafe {
                strong_shape.call_drop_in_place(*allocation);
            }
            // SAFETY: the allocation was created using Shape::alloc
            unsafe {
                strong_shape
                    .deallocate_mut(*allocation)
                    .expect("strong pointer is sized");
            }
        }
    }
}

/// Contains the guard, the data ptr, and drop vtable to free the lock
///
/// # Note
///
/// The contained [`MaybeMut`] is NOT guaranteed to be [`Mut`](MaybeMut::Mut)
///
/// For example, RwLock also needs a lock and guard for a read.
///
#[derive(Deref, DerefMut)]
pub struct Guard<'lock_mem, 'facet> {
    /// Dropping the guard handles freeing the lock
    ///
    /// If this is empty, the `data` can be accessed directly and there is no
    /// lock that must be freeed
    ///
    /// SAFETY: The pointer inside the [`LockGuardType`] MUST NOT be used
    /// since the data is already (mutable) available via `data`
    ///
    /// Drop order is relevant! The Guards are in the reverse order of their
    /// lifetime, meaning when dropping, from 0 to idx_max, guards
    /// with shorter lifetimes are dropped first
    guards: Vec<LockGuardType>,
    /// This [`MaybeMut`] contains the [`Peek`] or [`Poke`] of the most inner,
    /// non lockable type.
    #[deref]
    #[deref_mut]
    data: MaybeMut<'lock_mem, 'facet>,
}

impl<'mem, 'facet> MaybeMut<'mem, 'facet> {
    /// Try to turn [`MaybeMut::Not`] into [`MaybeMut::Mut`]
    ///
    /// The returned [`MaybeMut`] may contain a different [`Shape`].
    /// Which exact [`Shape`] it is, depends on what the input type was.
    ///
    /// One edge case is if you pass a `&mut Arc<RwLock<String>` the type will
    /// not be changed to `&mut String`. But if you pass a `&Arc<RwLock<String>`
    /// due to locking etc, it will be a `&mut String`.
    ///
    /// If the underlying type is something that can be write locked,
    /// for example an `RwLock` or `Mutex`, this method creates a lock on it.
    ///
    /// If we already have [`MaybeMut::Mut`] this is a no-op.
    ///
    /// If we have [`MaybeMut::Not`] and the [`Shape`] of
    /// `T` does not contain a [`PointerDef`](facet::PointerDef) which
    /// has a vtable with a `write_fn` we can call with `&T`, this method
    /// returns [`Err(MaybeMut::Not)`](Err). In this case, besides the lookup,
    /// it is also a no-op.
    ///
    /// # Note
    ///
    /// It is very important that you drop the [`Guard`] as soon as possible
    /// to free the lock
    ///
    /// [`Shape`]: facet::Shape
    pub fn write<'lock>(self) -> Result<Guard<'lock, 'facet>, MakeLockError<'mem, 'facet>>
    where
        'mem: 'lock,
    {
        match self {
            // if we already have a mut this is a no op
            MaybeMut::Mut(v) => {
                // but only if this is a type that is not a smart pointer that can be locked
                if let Def::Pointer(p) = v.as_peek().innermost_peek().shape().def
                // restrict downgrading to Peek only if there is a a lock _somewhere_
                    && (p.flags.contains(PointerFlags::LOCK) || p.flags.contains(PointerFlags::WEAK))
                {
                    Self::Not(v.into_peek()).write()
                } else {
                    Ok(Guard {
                        guards: Vec::new(),
                        data: v.into(),
                    })
                }
            }
            // this is where it gets interesting
            MaybeMut::Not(v) => {
                // SAFETY: v.innermost_peek() unwraps all transparent wrappers like Arc or Rc until something that needs
                // locking is reached which is all we care about
                // FIXME: naively using innermost_peek is a bad idea i think.
                // for example, in the UI if there is a NonZero<u32> this will peek
                // up tu u32. Then, we will perhaps display an editable u32 which
                // can be set to zero. Now what? We broke it boys
                let v = v.innermost_peek();
                // the shape of the pointer type (if it is one) but derefence smart pointers that can so without locking
                // e.g. Arc<T> AND also &T
                let shape = v.shape();
                let def = shape.def;

                // short cirucit if it is not a pointer. in these
                // cases we wont be able to reach something like
                // RwLock or Mutex
                let Def::Pointer(pointer) = def else {
                    return Err(MakeLockError {
                        unchanged: v,
                        kind: MakeLockErrorKind::NotLockable,
                    });
                };

                // we dont care if we lock it (Mutex) or write lock it (RwLock)
                let lock_fn =
                    pointer
                        .vtable
                        .write_fn
                        .or(pointer.vtable.lock_fn)
                        .ok_or(MakeLockError {
                            unchanged: v,
                            kind: MakeLockErrorKind::NotLockable,
                        });

                // SAFETY: v.innermost_peek() unwraps all transparent wrappers like Arc or Rc until something that needs
                // locking is reached which is also the same type we get the lock_fn from
                let (mut guards, mut value): (Vec<LockGuardType>, MaybeMut<'lock, 'facet>) =
                    match lock_fn {
                        Ok(lock_fn) => {
                            let res = unsafe { lock_fn(v.data()) };
                            let Ok(lock) = res else {
                                return Err(MakeLockError {
                                    unchanged: v,
                                    kind: MakeLockErrorKind::LockFailure,
                                });
                            };
                            // SAFETY: creates access via the PtrMut returned from locking
                            // the smart pointer. 'mem outlives 'lock this means
                            // the returned SmartPointer<'mem> also outlives the mutable Poke<'lock>
                            let poke: Poke<'lock, 'facet> = unsafe {
                                Poke::from_raw_parts(
                                    // if the input type was Arc<RwLock<String>> this willbe
                                    // a pointer to a String
                                    *lock.data(),
                                    shape
                                        .inner
                                        .expect("a smart pointer always has an inner shape"),
                                )
                            };

                            (vec![lock.into()], MaybeMut::Mut(poke))
                        }
                        // try it as an upgrade instead
                        Err(MakeLockError {
                            unchanged,
                            kind: MakeLockErrorKind::NotLockable,
                        }) if let Def::Pointer(pointer) = unchanged.shape().def
                            && let Some(upgrade_fn) = pointer.vtable.upgrade_into_fn
                            && let Some(strong_shape) =
                                def.into_pointer().ok().and_then(|x| x.strong()) =>
                        {
                            // if the strong shape is unsized, the Facet implementation of the type is wrong.
                            let strong = strong_shape
                                .allocate()
                                .expect("strong pointer is always sized");

                            // SAFETY: turning this peek into a PtrMut is okay,. because
                            // the upgrade function only needs &self
                            // in theory, the upgrade_fn signature could take a PtrConst as well.
                            let ptr = unsafe { v.data().into_mut() };
                            // SAFETY: Facet implementation of Weak garantees strong is the correct
                            // shape of the strong part for this Weak
                            let Some(guard) =
                                unsafe { upgrade_fn(ptr, strong) }.map(|strong_instance| {
                                    LockGuardType::Upgrade {
                                        strong_shape,
                                        allocation: strong_instance,
                                    }
                                })
                            else {
                                // SAFETY:
                                // [`UpgradeIntoFn`] guarantees that if None is returned,
                                // the strong pointer is not initialised
                                unsafe {
                                    strong_shape
                                        .deallocate_uninit(strong)
                                        .expect("is sized and allocated by Shape")
                                };
                                return Err(MakeLockError {
                                    kind: MakeLockErrorKind::NotUpgradable,
                                    unchanged: v,
                                });
                            };
                            // SAFETY: creates access via the PtrMut returned from locking
                            // the smart pointer. 'mem outlives 'lock this means
                            // the returned mutable Poke<'lock> lives shorter than 'mem
                            let peek: Peek<'lock, 'facet> = unsafe {
                                Peek::unchecked_new(
                                    guard.data_const(),
                                    shape
                                        .inner
                                        .expect("a smart pointer always has an inner shape"),
                                )
                            };

                            (vec![guard], MaybeMut::Not(peek.innermost_peek()))
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    };

                // unwrap remaining inner pointer types
                // -> all types that have an inner type and are a pointer
                while let Some(_inner) = value.as_peek().shape().inner
                    && let Def::Pointer(def) = value.as_peek().shape().def
                    // lock gets locked in the next write call
                    && (def.flags.contains(PointerFlags::LOCK) ||
                    // weak gets upgraded at the next write call
                    def.flags.contains(PointerFlags::WEAK) ||
                    // atomics just get unwrapped in the next write call
                    def.flags.contains(PointerFlags::ATOMIC))
                {
                    // SAFETY: we synthesize a Peek with the outer 'mem lifetime so we
                    // can re-enter `write`. The fabricated lifetime never escapes:
                    //
                    // * On success, the recursive call returns `Guard<'lock>`. Its
                    //   `data` is bounded by 'lock and its guards are moved into our
                    //   `guards` Vec, so the parent lock keeps the pointer live for
                    //   as long as the returned `Guard` exists.
                    // * On failure, we MUST NOT propagate the inner
                    //   `MakeLockError::unchanged` since that Peek carries the
                    //   fabricated 'mem lifetime while actually pointing into
                    //   lock-protected memory that we are about to release as our
                    //   `guards` Vec drops on early return. Instead we substitute
                    //   the original outer Peek `v`, which is genuinely valid for
                    //   'mem because it comes straight from the function input.
                    let shorter_peek: Peek<'mem, 'facet> =
                        unsafe { Peek::unchecked_new(value.as_peek().data(), value.shape()) };
                    let shorter_maybe: MaybeMut<'mem, 'facet> = MaybeMut::Not(shorter_peek);
                    let inner_guard: Guard<'lock, 'facet> = match shorter_maybe.write() {
                        Ok(g) => g,
                        Err(e) => {
                            // Peek v needs no Guards, drop them explicitly to free
                            // locks (they would be dropped by the return anyways)
                            // Discard `e.unchanged` (would dangle once `guards`
                            // drops on return)
                            drop(guards);
                            // substitute the original outer Peek
                            // which is actually valid for 'mem.
                            return Err(MakeLockError {
                                unchanged: v,
                                kind: e.kind,
                            });
                        }
                    };
                    // SAFETY: the values must be moved to the new vec not cloned and NOT dropped.
                    // this would lead to a double unlock later
                    let mut inner_guards = inner_guard.guards;
                    inner_guards.extend(guards);
                    guards = inner_guards;
                    // SAFETY: set the new value to the inner most available value
                    value = inner_guard.data;
                }
                Ok(Guard {
                    data: value,
                    guards,
                })
            }
        }
    }

    /// Returns a [`Guard`] with a lock that is sufficent for reading.
    ///
    /// In case of `RwLock` it is locked to read. If it is a `Mutex`, it must
    /// be exclusively locked to write but we only consider it being read which
    /// is safe
    pub fn read<'lock>(self) -> Result<Guard<'lock, 'facet>, MakeLockError<'mem, 'facet>>
    where
        'mem: 'lock,
    {
        let peek = self.into_peek();
        // unwrap smart pointers
        // this will deref Arcs but not Weaks, Weaks are handled special as a guard that automatically downgrades them on Drop
        let v = peek.innermost_peek();
        // the shape of the pointer type (if it is one) but derefence smart pointers that can so without locking
        // e.g. Arc<T>
        let shape = v.shape();
        let def = shape.def;

        // short cirucit if it is not a pointer. in these
        // cases we wont be able to reach something like
        // RwLock or Mutex
        // In this case, we just return the reference to the underlying type
        let Def::Pointer(pointer) = def else {
            return Ok(Guard {
                guards: Vec::new(),
                data: MaybeMut::Not(v),
            });
        };

        // we dont care if we lock it (Mutex) or read lock it (RwLock) or just upgrade it so it can be dereferenced
        let res: Result<LockGuardType, _> = if let Some(read_fn) = pointer.vtable.read_fn {
            unsafe { read_fn(v.data()) }.map(Into::into)
        } else if let Some(lock_fn) = pointer.vtable.lock_fn {
            unsafe { lock_fn(v.data()) }.map(Into::into)
            // handle weak pointers and try to upgrade them
        } else if let Some(upgrade_fn) = pointer.vtable.upgrade_into_fn
            && let Some(strong_shape) = def.into_pointer().ok().and_then(|x| x.strong())
        {
            // if the strong shape is unsized, the Facet implementation of the type is wrong.
            let strong = strong_shape
                .allocate()
                .expect("strong pointer is always sized");

            // SAFETY: turning this peek into a PtrMut is okay, because
            // the upgrade function only needs &self
            // in theory, the upgrade_fn signature could take a PtrConst as well.
            let ptr = unsafe { v.data().into_mut() };
            // SAFETY: Facet implementation of Weak garantees strong is the correct
            // shape of the strong part for this Weak
            let Some(guard) =
                unsafe { upgrade_fn(ptr, strong) }.map(|strong_instance| LockGuardType::Upgrade {
                    strong_shape,
                    allocation: strong_instance,
                })
            else {
                // SAFETY:
                // [`UpgradeIntoFn`] guarantees that if None is returned,
                // the strong pointer is not initialised
                unsafe {
                    strong_shape
                        .deallocate_uninit(strong)
                        .expect("is sized and allocated by Shape")
                };
                return Err(MakeLockError {
                    kind: MakeLockErrorKind::NotUpgradable,
                    unchanged: v,
                });
            };
            Ok(guard)
        } else {
            return Err(MakeLockError {
                unchanged: v,
                kind: MakeLockErrorKind::NotLockable,
            });
        };

        let Ok(lock) = res else {
            return Err(MakeLockError {
                unchanged: v,
                kind: MakeLockErrorKind::LockFailure,
            });
        };
        // SAFETY: creates access via the PtrMut returned from locking
        // the smart pointer. 'mem outlives 'lock this means
        // the returned mutable Poke<'lock> lives shorter than 'mem
        let peek: Peek<'lock, 'facet> = unsafe {
            Peek::unchecked_new(
                lock.data_const(),
                shape
                    .inner
                    .expect("a smart pointer always has an inner shape"),
            )
        };
        let (mut guards, mut value): (Vec<LockGuardType>, MaybeMut<'lock, 'facet>) =
            (vec![lock], MaybeMut::Not(peek.innermost_peek()));

        // unwrap remaining inner pointer types
        // -> all types that have an inner type and are a pointer
        while let Some(_inner) = value.as_peek().shape().inner
            && let Def::Pointer(def) = value.as_peek().shape().def
            // lock gets read-locked in the next read call
            && (def.flags.contains(PointerFlags::LOCK) ||
            // weak gets upgraded at the next read call
            def.flags.contains(PointerFlags::WEAK) ||
            // atomics just get unwrapped in the next read call
            def.flags.contains(PointerFlags::ATOMIC))
        {
            // SAFETY: we synthesize a Peek with the outer 'mem lifetime so we
            // can re-enter `read`. The fabricated lifetime never escapes:
            //
            // * On success, the recursive call returns `Guard<'lock>`. Its
            //   `data` is bounded by 'lock and its guards are moved into our
            //   `guards` Vec, so the parent lock keeps the pointer live for
            //   as long as the returned `Guard` exists.
            // * On failure, we MUST NOT propagate the inner
            //   `MakeLockError::unchanged` since that Peek carries the
            //   fabricated 'mem lifetime while actually pointing into
            //   lock-protected memory that we are about to release as our
            //   `guards` Vec drops on early return. Instead we substitute
            //   the original outer Peek `v`, which is genuinely valid for
            //   'mem because it comes straight from the function input.
            let shorter_peek: Peek<'mem, 'facet> =
                unsafe { Peek::unchecked_new(value.as_peek().data(), value.shape()) };
            let shorter_maybe: MaybeMut<'mem, 'facet> = MaybeMut::Not(shorter_peek);
            let inner_guard: Guard<'lock, 'facet> = match shorter_maybe.read() {
                Ok(g) => g,
                Err(e) => {
                    // Peek v needs no Guards, drop them explicitly to free
                    // locks (they would be dropped by the return anyways)
                    drop(guards);
                    // Discard `e.unchanged` (would dangle once `guards`
                    // drops on return); substitute the original outer Peek
                    // which is actually valid for 'mem.
                    return Err(MakeLockError {
                        unchanged: v,
                        kind: e.kind,
                    });
                }
            };
            // SAFETY: the values must be moved to the new vec not cloned and NOT dropped.
            // this would lead to a double unlock later
            let mut inner_guards = inner_guard.guards;
            inner_guards.extend(guards);
            guards = inner_guards;
            // SAFETY: set the new value to the inner most available value
            value = inner_guard.data;
        }
        Ok(Guard {
            data: value,
            guards,
        })
    }
}

#[cfg(test)]
mod tests {
    use facet::{Def, Facet, KnownPointer};
    use facet_reflect::Peek;

    #[derive(Debug, Facet)]
    struct Foo {
        value: String,
    }

    #[facet_testhelpers::test]
    fn shared_reference() {
        let a = Foo {
            value: "aaaa".to_string(),
        };
        println!("{:#?}", <&Foo as Facet<'_>>::SHAPE.def);
        assert!(
            matches!(<&Foo as Facet<'_>>::SHAPE.def, Def::Pointer(p) if p.known == Some(KnownPointer::SharedReference))
        );
        let ref_a: &Foo = &a;
        let ref_ref_a: &&Foo = &ref_a;
        let peek = Peek::new(ref_ref_a);
        println!("{:#?}", peek.shape().def); // `Undefined`
        assert!(
            matches!(peek.shape().def, Def::Pointer(p) if p.known == Some(KnownPointer::SharedReference))
        );
    }
}
