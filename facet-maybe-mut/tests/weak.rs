use std::sync::Arc;

use facet::Facet;
use facet_maybe_mut::MaybeMut;
use facet_reflect::Peek;

#[derive(Debug, Facet, Default)]
struct User {
    id: u32,
    name: String,
}

#[facet_testhelpers::test]
fn weak_arc_upgrade() {
    let strong = Arc::new(User {
        id: 3343,
        name: "Alejandro".to_owned(),
    });
    let original_weak = Arc::downgrade(&strong);
    assert_eq!(Arc::strong_count(&strong), 1);

    let weak = MaybeMut::Not(Peek::new(&original_weak)).read().unwrap();
    // the read lock is a lock for the upgrade of the weak thus during read,
    // another arc exists
    assert_eq!(
        Arc::strong_count(&strong),
        2,
        "the read lock upgraded the Weak to an Arc"
    );
    // in read innermost peek now borrows the inner shape which is User
    assert_eq!(weak.shape(), User::SHAPE);
    drop(strong);
    // the read guard hold an Arc internally
    assert_eq!(original_weak.strong_count(), 1);
    // this "weak" drop actual contains a "lock" to an Arc. The last Arc that is,
    // the orignal weak will now have no strong arcs left and the value is dropped
    drop(weak);
    assert_eq!(
        original_weak.strong_count(),
        0,
        "drop of the Guard calls the drop of the upgraded Arc thus decreasing strong count"
    );
    assert!(original_weak.upgrade().is_none());
}
