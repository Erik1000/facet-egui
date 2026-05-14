#![cfg(feature = "std")]

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use facet::Facet;
use facet_reflect::Peek;

use facet_egui::MaybeMut;

#[derive(Debug, Facet, Default)]
#[repr(C)]
enum Status {
    #[default]
    Single,
    Engaged,
    Married {
        // unix timestamp or something idk
        since: u128,
    },
}

#[derive(Debug, Facet)]
struct Being {
    name: String,
    age: u32,
    status: Status,
}

impl Being {
    pub fn born(name: String) -> Self {
        Self {
            age: 0,
            name,
            status: Status::default(),
        }
    }
}

#[test]
fn single_threaded() -> color_eyre::Result<()> {
    let value = Arc::new(RwLock::new(Being::born("Erik".to_string())));

    let not_mut_yet = MaybeMut::Not(Peek::new(&value));
    let mut now_mut = not_mut_yet.write().unwrap();
    match &mut *now_mut {
        MaybeMut::Mut(m) => {
            m.get_mut::<Being>().unwrap().age += 1;
        }
        MaybeMut::Not(..) => panic!("it should be mutable"),
    }

    Ok(())
}

#[test]
fn multi_thread() -> color_eyre::Result<()> {
    let amos = Being {
        age: 36, // forgive me please
        name: "Amos".to_string(),
        status: Status::Single,
    };
    let amos = Arc::new(RwLock::new(amos));

    let other_thread_amos = amos.clone();
    let handle = std::thread::spawn(move || {
        let amos = other_thread_amos;
        std::thread::sleep(Duration::from_secs(1));
        println!("getting engaged");
        std::thread::sleep(Duration::from_secs(2));
        let mut guard = MaybeMut::Not(Peek::new(&amos)).write().unwrap();
        if let MaybeMut::Mut(amos) = &mut *guard {
            amos.get_mut::<Being>().unwrap().status = Status::Engaged;
            println!("engaged!!!")
        }
        drop(guard);
        // time flies by
        std::thread::sleep(Duration::from_secs(5));
        println!("wedding is starting");
        let mut guard = MaybeMut::Not(Peek::new(&amos)).write().unwrap();
        if let MaybeMut::Mut(amos) = &mut *guard {
            amos.get_mut::<Being>().unwrap().status = Status::Married { since: 1771870398 };
            println!("married!!!!!!!!!!!!!!!!!")
        }
        println!("party party party");
        println!("thread done");
    });
    // this thread keeps watching
    println!("waiting until the wedding is over");
    loop {
        std::thread::sleep(Duration::from_secs(2));
        // this should only get locked to read in case of RwLock
        let not = MaybeMut::Not(Peek::new(&amos));
        let guard = not.read().unwrap();
        let status = &guard.as_peek().get::<Being>().unwrap().status;
        if let Status::Married { since } = status {
            handle.join().unwrap();
            println!("waiting is finally over, married since {since}");
            return Ok(());
        } else {
            println!("still waiting")
        }
    }
}
