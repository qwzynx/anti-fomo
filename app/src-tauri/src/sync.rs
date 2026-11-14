//! Locks that outlive a panic.
//!
//! A panic while a `std::sync` lock is held poisons it, and `.lock().unwrap()`
//! turns every later acquisition into a second panic. With the release build
//! unwinding (see `Cargo.toml`), a scraper that panics on a malformed page is
//! caught and reported as that source failing — but if it, or a write task,
//! happened to be holding the database lock, every command after it would
//! have panicked too, and the app would be bricked until restart.
//!
//! Recovering the guard is sound for everything locked here: the connection
//! rolls back an unfinished transaction when its guard is dropped during the
//! unwind, the ranked cache is replaced whole and never left half-built, and
//! the board lists are plain values assigned in one statement.

use std::sync::{Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub trait LockExt<T: ?Sized> {
    /// `lock()`, taking the guard back from a panicked holder.
    fn lock_recover(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> LockExt<T> for Mutex<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub trait RwLockExt<T: ?Sized> {
    fn read_recover(&self) -> RwLockReadGuard<'_, T>;
    fn write_recover(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T: ?Sized> RwLockExt<T> for RwLock<T> {
    fn read_recover(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(PoisonError::into_inner)
    }
    fn write_recover(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panicked_holder_does_not_brick_the_lock() {
        let m = std::sync::Arc::new(Mutex::new(1));
        let held = m.clone();
        let _ = std::thread::spawn(move || {
            let _guard = held.lock().unwrap();
            panic!("a scraper hit a page it could not parse");
        })
        .join();
        assert!(m.is_poisoned());
        *m.lock_recover() += 1;
        assert_eq!(*m.lock_recover(), 2);
    }
}
