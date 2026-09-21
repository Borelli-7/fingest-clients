//! Event bus adapter.
//!
//! Named after `InProcessPublisher` in `fingest-events`, and doing the same job one tier
//! down: fanning a fact out to everyone who registered an interest.
//!
//! Single-threaded by construction. The browser has one thread, so `Rc`/`RefCell` are the
//! honest primitives here; `Arc`/`Mutex` would buy nothing but atomics.

use std::cell::{Cell, RefCell};

use fingest_client_ports::{ClientEvent, EventBus, Subscriber, SubscriptionId};

#[derive(Default)]
pub struct InProcessBus {
    subscribers: RefCell<Vec<(SubscriptionId, Subscriber)>>,
    next_id: Cell<u64>,
}

impl InProcessBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.borrow().len()
    }
}

impl EventBus for InProcessBus {
    fn publish(&self, event: ClientEvent) {
        tracing::debug!(?event, "client event");

        // Snapshot before dispatching. A subscriber that subscribes, unsubscribes or
        // re-publishes while reacting is legitimate — and would panic on the live borrow.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn recorder() -> (Rc<RefCell<Vec<ClientEvent>>>, Subscriber) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        (
            seen,
            Rc::new(move |event: &ClientEvent| sink.borrow_mut().push(event.clone())),
        )
    }

    #[test]
    fn every_subscriber_sees_every_event() {
        let bus = InProcessBus::new();
        let (first, a) = recorder();
        let (second, b) = recorder();
        bus.subscribe(a);
        bus.subscribe(b);

        bus.publish(ClientEvent::SessionEnded);

        assert_eq!(first.borrow().len(), 1);
        assert_eq!(second.borrow().len(), 1);
    }

    #[test]
    fn unsubscribing_stops_delivery() {
        let bus = InProcessBus::new();
        let (seen, subscriber) = recorder();
        let id = bus.subscribe(subscriber);

        bus.publish(ClientEvent::SessionEnded);
        bus.unsubscribe(id);
        bus.publish(ClientEvent::SessionEnded);

        assert_eq!(seen.borrow().len(), 1);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn unsubscribing_an_unknown_id_is_harmless() {
        let bus = InProcessBus::new();
        bus.unsubscribe(SubscriptionId(99));
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn publishing_with_no_subscribers_is_a_no_op() {
        InProcessBus::new().publish(ClientEvent::SessionExpired);
    }

    /// A subscriber reacting by subscribing is the cache-invalidation pattern this bus
    /// exists for. It must not deadlock on the dispatch borrow.
    #[test]
    fn a_subscriber_may_subscribe_while_reacting() {
        let bus = Rc::new(InProcessBus::new());
        let inner = Rc::clone(&bus);
        let (seen, subscriber) = recorder();

        bus.subscribe(Rc::new(move |_: &ClientEvent| {
            inner.subscribe(subscriber.clone());
        }));

        bus.publish(ClientEvent::SessionEnded);

        assert_eq!(bus.subscriber_count(), 2);
        assert!(
            seen.borrow().is_empty(),
            "added mid-dispatch, so it missed this one"
        );
    }

    #[test]
    fn a_subscriber_may_unsubscribe_itself_while_reacting() {
        let bus = Rc::new(InProcessBus::new());
        let inner = Rc::clone(&bus);
        let id = Rc::new(Cell::new(SubscriptionId(0)));
        let held = Rc::clone(&id);

        id.set(bus.subscribe(Rc::new(move |_: &ClientEvent| {
            inner.unsubscribe(held.get());
        })));

        bus.publish(ClientEvent::SessionExpired);

        assert_eq!(bus.subscriber_count(), 0);
    }
}
