#![feature(plugin, custom_derive)]

#![plugin(gc_plugin)]
extern crate gc;

use std::cell::Cell;
use std::thread::LocalKey;
use gc::{Trace, GcCell, Gc, force_collect};

// Utility methods for the tests
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct GcWatchFlags {
    trace: i32,
    root: i32,
    unroot: i32,
    drop: i32,
}

impl GcWatchFlags {
    fn new(trace: i32, root: i32, unroot: i32, drop: i32) -> GcWatchFlags {
        GcWatchFlags {
            trace: trace,
            root: root,
            unroot: unroot,
            drop: drop,
        }
    }

    fn zero() -> Cell<GcWatchFlags> {
        Cell::new(GcWatchFlags {
            trace: 0,
            root: 0,
            unroot: 0,
            drop: 0,
        })
    }
}

struct GcWatch(&'static LocalKey<Cell<GcWatchFlags>>);

impl Drop for GcWatch {
    fn drop(&mut self) {
        self.0.with(|f| {
            let mut of = f.get();
            of.drop += 1;
            f.set(of);
        });
    }
}

impl Trace for GcWatch {
    fn trace(&self) {
        self.0.with(|f| {
            let mut of = f.get();
            of.trace += 1;
            f.set(of);
        });
    }
    fn root(&self) {
        self.0.with(|f| {
            let mut of = f.get();
            of.root += 1;
            f.set(of);
        });
    }
    fn unroot(&self) {
        self.0.with(|f| {
            let mut of = f.get();
            of.unroot += 1;
            f.set(of);
        });
    }
}

#[derive(Trace)]
struct GcWatchCycle {
    watch: GcWatch,
    cycle: GcCell<Option<Gc<GcWatchCycle>>>,
}

// Tests

#[test]
fn basic_allocate() {
    thread_local!(static FLAGS: Cell<GcWatchFlags> = GcWatchFlags::zero());

    {
        let _gced_val = Gc::new(GcWatch(&FLAGS));
        FLAGS.with(|f| assert_eq!(f.get(), GcWatchFlags::new(0, 0, 1, 0)));
        force_collect();
        FLAGS.with(|f| assert_eq!(f.get(), GcWatchFlags::new(1, 0, 1, 0)));
    }

    FLAGS.with(|f| assert_eq!(f.get(), GcWatchFlags::new(1, 0, 1, 0)));
    force_collect();
    FLAGS.with(|f| assert_eq!(f.get(), GcWatchFlags::new(1, 0, 1, 1)));
}

#[test]
fn basic_cycle_allocate() {
    thread_local!(static FLAGS1: Cell<GcWatchFlags> = GcWatchFlags::zero());
    thread_local!(static FLAGS2: Cell<GcWatchFlags> = GcWatchFlags::zero());

    {
        // Set up 2 nodes
        let node1 = Gc::new(GcWatchCycle {
            watch: GcWatch(&FLAGS1),
            cycle: GcCell::new(None),
        });
        FLAGS1.with(|f| assert_eq!(f.get(), GcWatchFlags::new(0, 0, 1, 0)));
        let node2 = Gc::new(GcWatchCycle {
            watch: GcWatch(&FLAGS2),
            cycle: GcCell::new(Some(node1.clone())),
        });

        FLAGS1.with(|f| assert_eq!(f.get(), GcWatchFlags::new(0, 0, 1, 0)));
        FLAGS2.with(|f| assert_eq!(f.get(), GcWatchFlags::new(0, 0, 1, 0)));

        force_collect();

        FLAGS1.with(|f| assert_eq!(f.get(), GcWatchFlags::new(1, 0, 1, 0)));
        FLAGS2.with(|f| assert_eq!(f.get(), GcWatchFlags::new(1, 0, 1, 0)));

        // Move node2 into the cycleref
        {
            *node1.cycle.borrow_mut() = Some(node2);

            FLAGS1.with(|f| assert_eq!(f.get(), GcWatchFlags::new(1, 0, 1, 0)));
            FLAGS2.with(|f| assert_eq!(f.get(), GcWatchFlags::new(1, 0, 1, 0)));

            force_collect();

            FLAGS1.with(|f| assert_eq!(f.get(), GcWatchFlags::new(2, 0, 1, 0)));
            FLAGS2.with(|f| assert_eq!(f.get(), GcWatchFlags::new(2, 0, 1, 0)));
        }

        FLAGS1.with(|f| assert_eq!(f.get(), GcWatchFlags::new(2, 0, 1, 0)));
        FLAGS2.with(|f| assert_eq!(f.get(), GcWatchFlags::new(2, 0, 1, 0)));

        force_collect();

        FLAGS1.with(|f| assert_eq!(f.get(), GcWatchFlags::new(3, 0, 1, 0)));
        FLAGS2.with(|f| assert_eq!(f.get(), GcWatchFlags::new(3, 0, 1, 0)));
    }

    FLAGS1.with(|f| assert_eq!(f.get(), GcWatchFlags::new(3, 0, 1, 0)));
    FLAGS2.with(|f| assert_eq!(f.get(), GcWatchFlags::new(3, 0, 1, 0)));

    force_collect();

    FLAGS1.with(|f| assert_eq!(f.get(), GcWatchFlags::new(3, 0, 1, 1)));
    FLAGS2.with(|f| assert_eq!(f.get(), GcWatchFlags::new(3, 0, 1, 1)));
}
