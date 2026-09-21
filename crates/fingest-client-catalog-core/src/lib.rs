//! Catalog bounded context, client side.
//!
//! Categories are shared reference data keyed by `(name, profit)`. Listing is public;
//! everything else is admin-only on the server, and this mirrors that so a non-admin is
//! never shown a control that would 403.

use std::rc::Rc;

use fingest_client_ports::{CatalogApi, ClientError, ClientEvent, EventBus};
use fingest_contracts::{CategoryDto, CreateCategoryRequest};

pub struct CatalogUseCase {
    api: Rc<dyn CatalogApi>,
    events: Rc<dyn EventBus>,
}

impl CatalogUseCase {
    pub fn new(api: Rc<dyn CatalogApi>, events: Rc<dyn EventBus>) -> Self {
        Self { api, events }
    }

    pub async fn list(&self) -> Result<Vec<CategoryDto>, ClientError> {
        self.api.list().await
    }

    pub async fn create(&self, name: &str, profit: bool) -> Result<CategoryDto, ClientError> {
        let name = validated_name(name)?;

        let created = self
            .api
            .create(CreateCategoryRequest { name, profit })
            .await?;
        self.events.publish(ClientEvent::CategoryChanged);

        Ok(created)
    }

    pub async fn rename(
        &self,
        name: &str,
        profit: bool,
        new_name: &str,
    ) -> Result<CategoryDto, ClientError> {
        let new_name = validated_name(new_name)?;
        if new_name == name {
            // The server would accept this and change nothing; saying so is more useful
            // than a silent success.
            return Err(ClientError::BadRequest(
                "The new name is the same as the old one".to_owned(),
            ));
        }

        let renamed = self.api.rename(name, profit, &new_name).await?;
        self.events.publish(ClientEvent::CategoryChanged);

        Ok(renamed)
    }

    pub async fn delete(&self, name: &str, profit: bool) -> Result<(), ClientError> {
        self.api.delete(name, profit).await?;
        self.events.publish(ClientEvent::CategoryChanged);

        Ok(())
    }
}

/// Trimmed and non-empty.
///
/// A blank name reaches the database as a valid-looking row that nothing can refer to, so
/// it is refused here rather than round-tripped.
fn validated_name(name: &str) -> Result<String, ClientError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ClientError::BadRequest(
            "Category name is required".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::StubCatalogApi;
    use fingest_client_ports::testing::RecordingBus;
    use futures::executor::block_on;

    fn use_case(api: Rc<StubCatalogApi>) -> (CatalogUseCase, Rc<RecordingBus>) {
        let events = Rc::new(RecordingBus::default());
        (CatalogUseCase::new(api, events.clone()), events)
    }

    fn category(name: &str) -> CategoryDto {
        CategoryDto {
            name: name.to_owned(),
            profit: false,
        }
    }

    #[test]
    fn creating_announces_that_pickers_are_stale() {
        let api = StubCatalogApi::returning(category("Food"));
        let (catalog, events) = use_case(api);

        block_on(catalog.create("Food", false)).unwrap();

        assert_eq!(events.recorded(), vec![ClientEvent::CategoryChanged]);
    }

    #[test]
    fn a_blank_name_never_reaches_the_network() {
        let api = StubCatalogApi::returning(category("Food"));
        let (catalog, events) = use_case(api.clone());

        assert!(block_on(catalog.create("   ", false)).is_err());
        assert_eq!(api.calls(), 0);
        assert!(events.recorded().is_empty());
    }

    #[test]
    fn a_name_is_trimmed_before_it_is_sent() {
        let api = StubCatalogApi::returning(category("Food"));
        let (catalog, _) = use_case(api.clone());

        block_on(catalog.create("  Food  ", false)).unwrap();

        assert_eq!(api.last_name(), Some("Food".to_owned()));
    }

    #[test]
    fn renaming_to_the_same_name_is_refused_rather_than_silently_accepted() {
        let api = StubCatalogApi::returning(category("Food"));
        let (catalog, _) = use_case(api.clone());

        assert!(block_on(catalog.rename("Food", false, "Food")).is_err());
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn a_rejected_create_announces_nothing() {
        let api = StubCatalogApi::failing(ClientError::Conflict("already exists".into()));
        let (catalog, events) = use_case(api);

        assert!(block_on(catalog.create("Food", false)).is_err());
        assert!(events.recorded().is_empty());
    }

    #[test]
    fn deleting_announces_that_pickers_are_stale() {
        let api = StubCatalogApi::returning(category("Food"));
        let (catalog, events) = use_case(api);

        block_on(catalog.delete("Food", false)).unwrap();

        assert_eq!(events.recorded(), vec![ClientEvent::CategoryChanged]);
    }

    /// A category in use answers 409 on the server (deviation D11). Nothing changed, so
    /// nothing is announced.
    #[test]
    fn deleting_a_category_in_use_announces_nothing() {
        let api = StubCatalogApi::failing(ClientError::Conflict("category in use".into()));
        let (catalog, events) = use_case(api);

        assert!(block_on(catalog.delete("Food", false)).is_err());
        assert!(events.recorded().is_empty());
    }
}
