use fingest_client_ports::ClientError;
use fingest_contracts::CategoryDto;
use fingest_kernel::CategoryRef;

use crate::describe;

/// What a category picker can offer, given how far its load has got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    pub options: Vec<CategoryDto>,
    /// Set when the load failed, so the form can say why the list is empty.
    pub error: Option<String>,
    /// Submit stays disabled until this is true.
    pub ready: bool,
}

pub fn picker(loaded: Option<&Result<Vec<CategoryDto>, ClientError>>) -> Picker {
    match loaded {
        Some(Ok(options)) => Picker {
            options: options.clone(),
            error: None,
            ready: true,
        },
        Some(Err(failure)) => Picker {
            options: Vec::new(),
            error: Some(format!(
                "Categories could not be loaded: {}",
                describe(failure)
            )),
            ready: false,
        },
        None => Picker {
            options: Vec::new(),
            error: None,
            ready: false,
        },
    }
}

/// A category is keyed by `(name, profit)` on the server, so any picker value must carry
/// both. The seed data has two rows named "Other" — a name-only value conflates them.
pub fn option_value(category: &CategoryDto) -> String {
    format!("{}|{}", category.profit, category.name)
}

pub fn find_category(options: &[CategoryDto], value: &str) -> Option<CategoryRef> {
    options
        .iter()
        .find(|category| option_value(category) == value)
        .map(|category| CategoryRef {
            name: category.name.clone(),
            profit: category.profit,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Vec<CategoryDto> {
        vec![
            CategoryDto {
                name: "Food".into(),
                profit: false,
            },
            CategoryDto {
                name: "Other".into(),
                profit: false,
            },
            CategoryDto {
                name: "Other".into(),
                profit: true,
            },
        ]
    }

    #[test]
    fn the_same_name_with_different_profit_resolves_distinctly() {
        let spending = find_category(&options(), "false|Other").unwrap();
        let income = find_category(&options(), "true|Other").unwrap();

        assert_eq!(spending.name, "Other");
        assert!(!spending.profit);
        assert!(income.profit);
    }

    #[test]
    fn an_unselected_category_resolves_to_nothing() {
        assert!(find_category(&options(), "").is_none());
    }

    #[test]
    fn an_unknown_value_resolves_to_nothing() {
        assert!(find_category(&options(), "false|Nope").is_none());
    }

    /// A name containing the separator must not be able to impersonate another row.
    #[test]
    fn a_name_containing_the_separator_does_not_collide() {
        let tricky = vec![CategoryDto {
            name: "a|b".into(),
            profit: false,
        }];

        assert!(find_category(&tricky, "false|a|b").is_some());
        assert!(find_category(&tricky, "false|a").is_none());
    }

    #[test]
    fn a_picker_is_not_ready_while_loading() {
        let state = picker(None);
        assert!(!state.ready);
        assert!(state.error.is_none());
    }

    /// A failed load used to become an empty list and a misleading "Choose a category".
    #[test]
    fn a_failed_load_explains_the_empty_picker() {
        let failed = Err(ClientError::Network("offline".into()));
        let state = picker(Some(&failed));

        assert!(!state.ready);
        assert!(state.options.is_empty());
        assert!(state.error.unwrap().contains("Could not reach the server"));
    }

    #[test]
    fn a_loaded_picker_offers_its_options() {
        let state = picker(Some(&Ok(options())));
        assert!(state.ready);
        assert_eq!(state.options.len(), 3);
    }
}
