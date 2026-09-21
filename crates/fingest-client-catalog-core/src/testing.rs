//! In-memory port doubles for the catalog.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use fingest_client_ports::{CatalogApi, ClientError};
use fingest_contracts::{CategoryDto, CreateCategoryRequest};

/// Answers every call the same way, and remembers what it was asked.
///
/// The call count is what proves a validation rule short-circuited before the network.
pub struct StubCatalogApi {
    outcome: Result<CategoryDto, ClientError>,
    calls: Cell<usize>,
    last_name: RefCell<Option<String>>,
}

impl StubCatalogApi {
    pub fn returning(category: CategoryDto) -> Rc<Self> {
        Rc::new(Self {
            outcome: Ok(category),
            calls: Cell::new(0),
            last_name: RefCell::new(None),
        })
    }

    pub fn failing(error: ClientError) -> Rc<Self> {
        Rc::new(Self {
            outcome: Err(error),
            calls: Cell::new(0),
            last_name: RefCell::new(None),
        })
    }

    pub fn calls(&self) -> usize {
        self.calls.get()
    }

    pub fn last_name(&self) -> Option<String> {
        self.last_name.borrow().clone()
    }

    fn record(&self, name: Option<&str>) {
        self.calls.set(self.calls.get() + 1);
        if let Some(name) = name {
            *self.last_name.borrow_mut() = Some(name.to_owned());
        }
    }
}

#[async_trait(?Send)]
impl CatalogApi for StubCatalogApi {
    async fn list(&self) -> Result<Vec<CategoryDto>, ClientError> {
        self.record(None);
        self.outcome.clone().map(|category| vec![category])
    }

    async fn create(&self, request: CreateCategoryRequest) -> Result<CategoryDto, ClientError> {
        self.record(Some(&request.name));
        self.outcome.clone()
    }

    async fn rename(&self, _: &str, _: bool, new_name: &str) -> Result<CategoryDto, ClientError> {
        self.record(Some(new_name));
        self.outcome.clone()
    }

    async fn delete(&self, name: &str, _: bool) -> Result<(), ClientError> {
        self.record(Some(name));
        self.outcome.clone().map(|_| ())
    }
}
