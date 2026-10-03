//! The one mutex type every no_std crate uses. Hosted (`std` feature) it is
//! parking_lot's blocking mutex; bare-metal it is a spinlock. Both lock with
//! `.lock()` returning a guard directly, and neither can be poisoned.

#[cfg(feature = "std")]
pub use parking_lot::{Mutex, MutexGuard};
// Spin fallback is only safe for cooperative/same-priority scheduling; see spec section 7 Risks.
#[cfg(not(feature = "std"))]
pub use spin::{Mutex, MutexGuard};

#[cfg(test)]
mod tests {
    use super::Mutex;

    #[test]
    fn lock_gives_mutable_access() {
        let m = Mutex::new(1);
        *m.lock() += 1;
        assert_eq!(*m.lock(), 2);
    }
}
