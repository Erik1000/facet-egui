use std::sync::{Arc, RwLock, Weak};

use facet::Facet;
use facet_maybe_mut::MaybeMut;
use facet_reflect::{Peek, Poke};

#[derive(Debug, Facet, Default)]
struct User {
    id: u32,
    name: String,
    parent: Weak<RwLock<User>>,
}

#[test]
fn smoke() {
    let parent = Arc::new(RwLock::new(User {
        id: 1432,
        name: "parent of user".to_string(),
        // has no parent
        parent: Weak::new(),
    }));

    let mut strong = Arc::new(RwLock::new(User {
        id: 123,
        name: "im a child".to_string(),
        parent: Arc::downgrade(&parent),
    }));

    let w = Arc::downgrade(&strong);

    let new_parent = User {
        id: 95,
        name: "Other parent".to_string(),
        parent: Weak::new(),
    };

    assert_eq!(w.strong_count(), 1);
    {
        let mut guard = MaybeMut::Not(Peek::new(&w)).write().unwrap();
        let mut poke = guard.as_poke().unwrap();
        let w_user = poke.get_mut::<User>().unwrap();
        w_user.parent.upgrade().unwrap().write().unwrap().name = "changed".to_string();
    }
    assert_eq!(
        strong
            .read()
            .unwrap()
            .parent
            .upgrade()
            .unwrap()
            .read()
            .unwrap()
            .name,
        "changed"
    );

    {
        let mut guard = MaybeMut::Mut(Poke::new(&mut strong)).write().unwrap();
        let poke = guard.as_poke().unwrap();
        let child = poke.get::<User>().unwrap();
        let mut old_parent = MaybeMut::Not(Peek::new(&child.parent)).write().unwrap();
        old_parent.as_poke().unwrap().set(new_parent).unwrap();
    }
    assert_eq!(w.strong_count(), 1);

    assert_eq!(
        strong
            .read()
            .unwrap()
            .parent
            .upgrade()
            .unwrap()
            .read()
            .unwrap()
            .name,
        "Other parent"
    );

    drop(strong);

    // this wont work which is correct, otherwise it would be UB
    //let user = bad.as_peek().get::<User>().unwrap();
    assert_eq!(w.strong_count(), 0);
}
