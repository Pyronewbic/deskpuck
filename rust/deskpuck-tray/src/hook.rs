//! A callback that drops a value from outside its owner's scope.

use std::cell::RefCell;
use std::rc::Rc;

/// Takes the value out of `cell` when called, if it is still there. Holds only
/// a weak reference, so unwinding out of the owner still drops the value in
/// place instead of leaving it to process exit.
pub fn release_hook<T: 'static>(cell: &Rc<RefCell<Option<T>>>) -> Box<dyn FnOnce()> {
    let cell = Rc::downgrade(cell);
    Box::new(move || {
        if let Some(cell) = cell.upgrade()
            && let Ok(mut value) = cell.try_borrow_mut()
        {
            value.take();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Counted(Rc<Cell<u32>>);

    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn calling_the_hook_drops_the_value_once() {
        let drops = Rc::new(Cell::new(0));
        let cell = Rc::new(RefCell::new(Some(Counted(Rc::clone(&drops)))));
        release_hook(&cell)();
        assert_eq!(drops.get(), 1);
        assert!(cell.borrow().is_none());
        drop(cell);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn the_hook_does_not_keep_the_value_alive() {
        let drops = Rc::new(Cell::new(0));
        let cell = Rc::new(RefCell::new(Some(Counted(Rc::clone(&drops)))));
        let hook = release_hook(&cell);
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owner = cell;
            panic!("owner unwinds");
        }));
        assert!(unwound.is_err());
        assert_eq!(drops.get(), 1, "dropped during the unwind, not at exit");
        hook();
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn a_busy_value_is_left_alone() {
        let drops = Rc::new(Cell::new(0));
        let cell = Rc::new(RefCell::new(Some(Counted(Rc::clone(&drops)))));
        let borrowed = cell.borrow();
        release_hook(&cell)();
        assert_eq!(drops.get(), 0);
        drop(borrowed);
    }
}
