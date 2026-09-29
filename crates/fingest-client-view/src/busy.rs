use dioxus::prelude::*;

use crate::NOT_SIGNED_IN;

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

/// Anything that can say whether an action is already running.
///
/// A trait rather than `Signal<bool>` directly so the rules below are testable on the host,
/// where no Dioxus runtime exists to own a signal.
pub trait BusyFlag: Clone + 'static {
    fn is_raised(&self) -> bool;
    fn raise(&mut self);
    fn lower(&mut self);
}

impl BusyFlag for Signal<bool> {
    fn is_raised(&self) -> bool {
        // `peek`: a click handler must not subscribe the component to its own flag.
        self.try_peek().is_ok_and(|flag| *flag)
    }

    fn raise(&mut self) {
        self.set(true);
    }

    fn lower(&mut self) {
        // The component may already be unmounted, taking the signal with it.
        if let Ok(mut flag) = self.try_write() {
            *flag = false;
        }
    }
}

fn guard<F: BusyFlag>(mut flag: F) -> BusyGuard<impl FnOnce()> {
    flag.raise();
    BusyGuard::new(move || flag.lower())
}

/// Marks a form busy now, before its task is spawned, and returns the guard that frees it.
///
/// Move the guard into the task: it drops when the task finishes, returns early or is
/// cancelled with its component.
pub fn hold(busy: Signal<bool>) -> BusyGuard<impl FnOnce()> {
    guard(busy)
}

/// Why an action did not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The same action is still in flight: a double click, not a second request.
    InFlight,
    /// There is no session to act with.
    SignedOut,
}

impl Refused {
    /// What to show, if anything. A repeated click is ignored silently.
    pub fn message(self) -> Option<&'static str> {
        match self {
            Self::InFlight => None,
            Self::SignedOut => Some(NOT_SIGNED_IN),
        }
    }
}

/// Claims `busy` for an action that needs no session.
///
/// `None` while the flag is already raised, so a second click does not send a second
/// request.
pub fn claim<F: BusyFlag>(busy: F) -> Option<BusyGuard<impl FnOnce()>> {
    if busy.is_raised() {
        return None;
    }
    Some(guard(busy))
}

/// Starts an action on behalf of the signed-in user, synchronously, before `spawn`.
///
/// Refuses while the same action is in flight, and refuses visibly when there is no
/// session instead of letting a spawned task return without a word. On success the guard
/// must be moved into the task, so every exit path of that task releases the flag.
pub fn begin<F: BusyFlag, S>(
    busy: F,
    session: Option<S>,
) -> Result<(BusyGuard<impl FnOnce()>, S), Refused> {
    if busy.is_raised() {
        return Err(Refused::InFlight);
    }
    let Some(session) = session else {
        return Err(Refused::SignedOut);
    };
    Ok((guard(busy), session))
}

/// [`begin`] for a component: reports a refusal in `error`, and clears a stale error when
/// the action does start.
///
/// Call it in the event handler, before `spawn`, and move the guard into the task.
pub fn start_action<S>(
    busy: Signal<bool>,
    mut error: Signal<Option<String>>,
    session: Option<S>,
) -> Option<(BusyGuard<impl FnOnce()>, S)> {
    match begin(busy, session) {
        Ok(started) => {
            if error.peek().is_some() {
                error.set(None);
            }
            Some(started)
        }
        Err(refused) => {
            if let Some(message) = refused.message() {
                error.set(Some(message.to_owned()));
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct Flag(Rc<Cell<bool>>);

    impl BusyFlag for Flag {
        fn is_raised(&self) -> bool {
            self.0.get()
        }
        fn raise(&mut self) {
            self.0.set(true);
        }
        fn lower(&mut self) {
            self.0.set(false);
        }
    }

    /// Stands in for a spawned task: it owns the guard and may end any of three ways.
    fn run_task<G>(
        guard: G,
        outcome: Result<(), &'static str>,
        bail_early: bool,
    ) -> Result<(), &'static str> {
        let _busy = guard;
        if bail_early {
            return Err("no longer relevant");
        }
        outcome
    }

    #[test]
    fn a_second_trigger_is_refused_while_the_first_is_in_flight() {
        let flag = Flag::default();

        let (first, _) = begin(flag.clone(), Some("session")).unwrap();
        assert!(flag.is_raised());
        assert_eq!(
            begin(flag.clone(), Some("session")).err(),
            Some(Refused::InFlight)
        );
        assert!(claim(flag.clone()).is_none());

        drop(first);
        assert!(!flag.is_raised());
        assert!(begin(flag.clone(), Some("session")).is_ok());
    }

    #[test]
    fn a_repeated_trigger_is_ignored_without_a_message() {
        assert_eq!(Refused::InFlight.message(), None);
    }

    #[test]
    fn a_missing_session_is_refused_visibly_and_leaves_the_flag_down() {
        let flag = Flag::default();

        let refused = begin(flag.clone(), None::<&str>).err();

        assert_eq!(refused, Some(Refused::SignedOut));
        assert_eq!(Refused::SignedOut.message(), Some(NOT_SIGNED_IN));
        assert!(!flag.is_raised());
    }

    #[test]
    fn the_flag_is_released_on_success_error_and_early_exit() {
        for (outcome, bail_early) in [
            (Ok(()), false),
            (Err("server said no"), false),
            (Ok(()), true),
        ] {
            let flag = Flag::default();
            let (guard, _) = begin(flag.clone(), Some("session")).unwrap();

            let _ = run_task(guard, outcome, bail_early);

            assert!(
                !flag.is_raised(),
                "left raised for {outcome:?}, bail_early={bail_early}"
            );
        }
    }

    #[test]
    fn a_claim_needs_no_session_and_releases_on_drop() {
        let flag = Flag::default();

        let held = claim(flag.clone()).unwrap();
        assert!(flag.is_raised());
        assert!(claim(flag.clone()).is_none());

        drop(held);
        assert!(!flag.is_raised());
    }

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
