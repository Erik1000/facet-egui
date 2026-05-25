use facet::Facet;
use facet_maybe_mut::MaybeMut;
use facet_reflect::Peek;
use std::sync::{Arc, RwLock, Weak};

#[derive(Debug, Facet, Default)]
struct SharedState {
    message: String,
    count: u32,
}

fn main() {
    println!("facet-maybe-mut demo: write() and read() with Arc/Weak + RwLock");

    // Strong Arc to an RwLock-protected SharedState
    let strong = Arc::new(RwLock::new(SharedState {
        message: "hello".into(),
        count: 1,
    }));

    // A weak pointer to the same allocation
    let weak: Weak<RwLock<SharedState>> = Arc::downgrade(&strong);

    // Create a MaybeMut from a Peek of &Arc<RwLock<SharedState>> and try to write-lock it
    let peek_arc = Peek::new(&strong);
    let mm_arc: MaybeMut<'_, '_> = MaybeMut::Not(peek_arc);
    match mm_arc.write() {
        Ok(_guard) => println!("write() on Arc<RwLock<...>> succeeded (obtained mutable access)"),
        Err(e) => println!("write() failed: {}", e),
    }

    // Create a MaybeMut from a Peek of &Weak<RwLock<SharedState>> and try to read (upgrade)
    let peek_weak = Peek::new(&weak);
    let mm_weak: MaybeMut<'_, '_> = MaybeMut::Not(peek_weak);
    match mm_weak.read() {
        Ok(_guard) => println!("read() on Weak upgraded to strong and acquired read access"),
        Err(e) => println!("read() (upgrade) failed: {}", e),
    }

    // Drop the only strong Arc and show that upgrade fails
    drop(strong);
    let peek_weak2 = Peek::new(&weak);
    let mm_weak2: MaybeMut<'_, '_> = MaybeMut::Not(peek_weak2);
    match mm_weak2.read() {
        Ok(_) => println!("unexpected: upgrade succeeded after strong was dropped"),
        Err(e) => println!("expected failure after drop: {}", e),
    }
}
