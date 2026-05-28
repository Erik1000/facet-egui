use std::sync::{Arc, RwLock};

use facet::Facet;
use facet_maybe_mut::MaybeMut;
use facet_reflect::Peek;

#[derive(Debug, Facet, Default)]
struct User {
    id: u32,
    name: String,
}

#[test]
fn smoke() {
    let strong = Arc::new(RwLock::new(User {
        id: 123,
        name: String::new(),
    }));
    let w = Arc::downgrade(&strong);
    let dummy = User {
        id: 787,
        name: String::new(),
    };

    let mut guard: facet_maybe_mut::Guard<'_, '_> = MaybeMut::Not(Peek::new(&w)).read().unwrap();
    let bad: MaybeMut<'_, '_> =
        std::mem::replace(&mut guard.as_maybe(), MaybeMut::Not(Peek::new(&dummy)));

    drop(strong);
    //drop(guard);

    assert_eq!(w.strong_count(), 1);

    let user = bad.as_peek().get::<User>().unwrap();
    assert_eq!(user.id, 123);
    drop(guard);
    // this wont work which is correct, otherwise it would be UB
    //let user = bad.as_peek().get::<User>().unwrap();
    assert_eq!(w.strong_count(), 0);
}
