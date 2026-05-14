use derive_more::{Deref, DerefMut, From};
use facet::{Def, PointerFlags, PtrConst, ReadLockResult, Shape, WriteLockResult};
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
}

/// Depending on whether this is a read or write lock, `P` will be either
/// [`PtrConst`] or [`PtrMut`]. This enum makes `P` dynamic
#[derive(From)]
pub(crate) enum LockGuardType {
    Write(WriteLockResult),
    Read(ReadLockResult),
}

impl LockGuardType {
    pub fn data_const(&self) -> PtrConst {
        match self {
            Self::Write(w) => w.data_const(),
            Self::Read(r) => *r.data(),
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
    /// If this is None, the `data` can be accessed directly and there is no
    /// lock that must be freeed
    ///
    /// SAFETY: The pointer inside the [`LockGuardType`] MUST NOT be used
    /// since the data is already (mutable) available via `data`
    _guard: Option<LockGuardType>,
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
                    && p.flags.contains(PointerFlags::LOCK)
                {
                    Self::Not(v.into_peek()).write()
                } else {
                    Ok(Guard {
                        _guard: None,
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
                let lock_fn = match (pointer.vtable.write_fn, pointer.vtable.lock_fn) {
                    (Some(write_fn), _) => write_fn,
                    (_, Some(lock_fn)) => lock_fn,
                    _ => {
                        return Err(MakeLockError {
                            unchanged: v,
                            kind: MakeLockErrorKind::NotLockable,
                        });
                    }
                };
                // SAFETY: v.innermost_peek() unwraps all transparent wrappers like Arc or Rc until something that needs
                // locking is reached which is also the same type we get the lock_fn from
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
                let value: MaybeMut<'lock, 'facet> = MaybeMut::Mut(poke);
                Ok(Guard {
                    data: value,
                    _guard: Some(LockGuardType::Write(lock)),
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
                _guard: None,
                data: MaybeMut::Not(v),
            });
        };

        // we dont care if we lock it (Mutex) or read lock it (RwLock)
        let res: Result<LockGuardType, _> = if let Some(read_fn) = pointer.vtable.read_fn {
            unsafe { read_fn(v.data()) }.map(Into::into)
        } else if let Some(lock_fn) = pointer.vtable.lock_fn {
            unsafe { lock_fn(v.data()) }.map(Into::into)
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
        // the smart pointer. 'lock outlives 'mem this means
        // the returned mutable Poke<'lock> also outlives the SmartPointer<'mem>
        let peek: Peek<'lock, 'facet> = unsafe {
            Peek::unchecked_new(
                lock.data_const(),
                shape
                    .inner
                    .expect("a smart pointer always has an inner shape"),
            )
        };
        let value = MaybeMut::Not(peek);
        Ok(Guard {
            _guard: Some(lock),
            data: value,
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
