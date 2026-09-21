//! In-memory port doubles.
//!
//! Real implementations, not mocks — the same choice the API workspace makes in
//! `fingest-identity-core::testing`. A double that records and replays is easier to reason
//! about than an expectation DSL, and it cannot drift from the trait it implements.
//!
//! Only doubles for ports declared *here* live in this module. A stub for a specific API
//! port belongs with the context that owns its use cases.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::{
    ClientEvent, EventBus, Session, SessionStore, Subscriber, SubscriptionId, TokenSource,
};

/// Records what was published, so a test can assert on announcements and not just on state.
#[derive(Default)]
pub struct RecordingBus {
    events: RefCell<Vec<ClientEvent>>,
    subscribers: RefCell<Vec<(SubscriptionId, Subscriber)>>,
    next_id: Cell<u64>,
}

impl RecordingBus {
    pub fn recorded(&self) -> Vec<ClientEvent> {
        self.events.borrow().clone()
    }
}

impl EventBus for RecordingBus {
    fn publish(&self, event: ClientEvent) {
        self.events.borrow_mut().push(event.clone());

        // Cloned out before dispatching: a subscriber that subscribes or unsubscribes while
        // reacting would otherwise panic on the outstanding borrow.
        let subscribers: Vec<Subscriber> = self
            .subscribers
            .borrow()
            .iter()
            .map(|(_, subscriber)| subscriber.clone())
            .collect();

        for subscriber in subscribers {
            subscriber(&event);
        }
    }

    fn subscribe(&self, subscriber: Subscriber) -> SubscriptionId {
        let id = SubscriptionId(self.next_id.get());
        self.next_id.set(id.0 + 1);
        self.subscribers.borrow_mut().push((id, subscriber));
        id
    }

    fn unsubscribe(&self, id: SubscriptionId) {
        self.subscribers
            .borrow_mut()
            .retain(|(held, _)| *held != id);
    }
}

/// A session store backed by nothing but a cell.
#[derive(Default)]
pub struct InMemorySessionStore(RefCell<Option<Session>>);

impl InMemorySessionStore {
    pub fn with(session: Session) -> Rc<Self> {
        Rc::new(Self(RefCell::new(Some(session))))
    }
}

impl SessionStore for InMemorySessionStore {
    fn load(&self) -> Option<Session> {
        self.0.borrow().clone()
    }

    fn save(&self, session: &Session) {
        *self.0.borrow_mut() = Some(session.clone());
    }

    fn clear(&self) {
        *self.0.borrow_mut() = None;
    }
}

impl TokenSource for InMemorySessionStore {
    fn token(&self) -> Option<String> {
        self.load().map(|session| session.token)
    }
}
