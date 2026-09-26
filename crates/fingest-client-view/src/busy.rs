use dioxus::prelude::*;

/// Runs `release` when dropped, so every exit path from a submit task clears the flag.
pub struct BusyGuard<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> BusyGuard<F> {
    pub fn new(release: F) -> Self {
        Self(Some(release))
    }
}

impl<F: FnOnce()> Drop for BusyGuard<F> {
    fn drop(&mut self) {
        if let Some(release) = self.0.take() {
            release();
        }
    }
}

/// Marks a form busy now, before its task is spawned, and returns the guard that frees it.
///
/// Move the guard into the task: it drops when the task finishes, returns early or is
/// cancelled with its component.
pub fn hold(mut busy: Signal<bool>) -> BusyGuard<impl FnOnce()> {
    busy.set(true);
    BusyGuard::new(move || {
        // The form may already be unmounted, taking the signal with it.
        if let Ok(mut flag) = busy.try_write() {
            *flag = false;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn submit(released: &Cell<u32>, bail_early: bool) -> Result<(), &'static str> {
        let _guard = BusyGuard::new(|| released.set(released.get() + 1));
        if bail_early {
            return Err("no session");
        }
        Ok(())
    }

    #[test]
    fn a_finished_task_releases_the_form() {
        let released = Cell::new(0);
        submit(&released, false).unwrap();
        assert_eq!(released.get(), 1);
    }

    /// The bug this exists for: an early `return` used to skip the reset.
    #[test]
    fn an_early_return_still_releases_the_form() {
        let released = Cell::new(0);
        assert!(submit(&released, true).is_err());
        assert_eq!(released.get(), 1);
    }

    #[test]
    fn nothing_is_released_while_the_guard_is_held() {
        let released = Cell::new(0);
        let guard = BusyGuard::new(|| released.set(released.get() + 1));
        assert_eq!(released.get(), 0);
        drop(guard);
        assert_eq!(released.get(), 1);
    }
}
