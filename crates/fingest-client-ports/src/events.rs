use std::rc::Rc;

/// Something that happened in the browser that more than one part of the UI cares about.
///
/// Names mirror `fingest_kernel::DomainEvent` where a counterpart exists, so a client log
/// line and a server log line describing the same action read the same. The session
/// variants have no server counterpart: a token expiring is a client-side fact.
///
/// `#[non_exhaustive]` because later phases add wallet, expense and budget variants.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClientEvent {
    /// A login succeeded, or a stored session was restored on reload.
    SessionStarted {
        login: String,
        admin: bool,
    },

    /// The user logged out deliberately.
    SessionEnded,

    /// A call came back 401. Published by the transport, not by a use case.
    SessionExpired,

    /// Shared reference data changed, so every category picker is stale.
    CategoryChanged,

    /// The wallet list, and any balance shown beside it, are stale.
    WalletChanged,

    /// An expense was added, edited or removed in `wallet_id`.
    ///
    /// Carries the wallet because a change there invalidates three separate reads — the
    /// expense list, the summary and the balance — that no single view owns.
    ExpenseChanged {
        wallet_id: i32,
    },

    /// A budget was created, edited or removed.
    BudgetChanged,

    /// An account was edited or removed. Carries the login so a list can target one row.
    AccountChanged {
        login: String,
    },

    AccountRemoved {
        login: String,
    },

    AccountRegistered {
        login: String,
    },
}

pub type Subscriber = Rc<dyn Fn(&ClientEvent)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubscriptionId(pub u64);

/// Fan-out inside the browser.
///
/// This exists so a use case can announce a fact without knowing who reacts — an expense
/// form does not know a budget card exists. Resist adding an event with exactly one
/// subscriber; that is a function call wearing a costume.
pub trait EventBus {
    fn publish(&self, event: ClientEvent);

    fn subscribe(&self, subscriber: Subscriber) -> SubscriptionId;

    fn unsubscribe(&self, id: SubscriptionId);
}
