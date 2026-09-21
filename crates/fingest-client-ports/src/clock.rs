use chrono::NaiveDate;

/// Today, according to the browser.
///
/// A port rather than a direct `chrono::Local::now()` call so date-defaulting logic in the
/// cores stays testable without freezing the machine clock. Mirrors `fingest_kernel::Clock`.
pub trait Clock {
    fn today(&self) -> NaiveDate;
}
