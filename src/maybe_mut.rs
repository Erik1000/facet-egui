use derive_more::{Deref, DerefMut, From};
use facet::{Def, WriteLockResult};
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
    pub fn as_peek(&'mem self) -> Peek<'mem, 'facet> {
        match self {
            Self::Not(peek) => *peek,
            Self::Mut(poke) => poke.as_peek(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct MakeMutError<'mem, 'facet> {
    pub unchanged: Peek<'mem, 'facet>,
    pub kind: MakeMutErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub enum MakeMutErrorKind {
    /// The underlying is not a pointer type (see [`KnownPointer`](facet::KnownPointer))
    #[error("type is not a pointer")]
    NoPointer,
    /// The underlying type is not a type that we can lock from a `&T` to `&mut T`
    #[error("type cannot be locked")]
    NotLockable,
    /// The underlying type could be locked but the provided lock method in the
    /// vtable returned an error.
    #[error("locking of type failed")]
    LockFailure,
}

/// contains the guard, the data ptr, and drop vtable to free the lock
///
/// # Note
///
/// The contained [`MaybeMut`] is guaranteed to be [`Mut`](MaybeMut::Mut)
#[derive(Deref, DerefMut)]
pub struct Guard<'lock_mem, 'facet> {
    // dropping the guard handles freeing the lock
    // if this is None, the `data` can be accessed directly and there is no
    // lock that must be freeed
    _guard: Option<WriteLockResult>,
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
    pub fn make_mut<'lock>(self) -> Result<Guard<'lock, 'facet>, MakeMutError<'mem, 'facet>>
    where
        'mem: 'lock,
    {
        match self {
            // if we already have a mut this is a no op
            MaybeMut::Mut(v) => Ok(Guard {
                _guard: None,
                data: v.into(),
            }),
            // this is where it gets interesting
            MaybeMut::Not(v) => {
                // SAFETY: v.innermost_peek() unwraps all transparent wrappers like Arc or Rc until something that needs
                // locking is reached which is all we care about
                let v = v.innermost_peek();
                // the shape of the pointer type (if it is one) but derefence smart pointers that can so without locking
                // e.g. Arc<T>
                let shape = v.shape();
                let def = shape.def;

                // short cirucit if it is not a pointer. in these
                // cases we wont be able to reach something like
                // RwLock or Mutex
                let Def::Pointer(pointer) = def else {
                    return Err(MakeMutError {
                        unchanged: v,
                        kind: MakeMutErrorKind::NoPointer,
                    });
                };

                // we dont care if we lock it (Mutex) or write lock it (RwLock)
                let lock_fn = match (pointer.vtable.write_fn, pointer.vtable.lock_fn) {
                    (Some(write_fn), _) => write_fn,
                    (_, Some(lock_fn)) => lock_fn,
                    _ => {
                        return Err(MakeMutError {
                            unchanged: v,
                            kind: MakeMutErrorKind::NotLockable,
                        });
                    }
                };
                // SAFETY: v.innermost_peek() unwraps all transparent wrappers like Arc or Rc until something that needs
                // locking is reached which is also the same type we get the lock_fn from
                let res = unsafe { lock_fn(v.data()) };
                let Ok(lock) = res else {
                    return Err(MakeMutError {
                        unchanged: v,
                        kind: MakeMutErrorKind::LockFailure,
                    });
                };

                // SAFETY: creates access via the PtrMut returned from locking
                // the smart pointer. 'lock outlives 'mem this means
                // the returned mutable Poke<'lock> also outlives the SmartPointer<'mem>
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
                let value = MaybeMut::Mut(poke);
                Ok(Guard {
                    data: value,
                    _guard: Some(lock),
                })
            }
        }
    }
}
