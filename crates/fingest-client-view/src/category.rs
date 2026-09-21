use fingest_contracts::CategoryDto;
use fingest_kernel::CategoryRef;

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
}
