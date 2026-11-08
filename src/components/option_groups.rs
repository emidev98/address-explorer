//! Shared grouped-option ordering for the filter and catalog pickers.
use std::collections::BTreeMap;
/// Group options by their display group, keeping the last-label group at the
/// end and ordering the other groups alphabetically.
pub fn group_options<T: Clone>(
    items: &[T],
    group_of: impl Fn(&T) -> String,
    last_label: &str,
) -> Vec<(String, Vec<T>)> {
    let mut groups: BTreeMap<String, Vec<T>> = BTreeMap::new();
    for item in items {
        groups.entry(group_of(item)).or_default().push(item.clone());
    }
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|(left, _), (right, _)| {
        let left_last = left.eq_ignore_ascii_case(last_label);
        let right_last = right.eq_ignore_ascii_case(last_label);
        left_last
            .cmp(&right_last)
            .then_with(|| left.to_lowercase().cmp(&right.to_lowercase()))
    });
    groups
}
#[cfg(test)]
mod tests {
    use super::group_options;
    #[test]
    fn groups_are_alphabetical_and_the_last_label_goes_last() {
        let items = [
            ("Binance", "Beta"),
            ("Kraken", "Gamma"),
            ("Acme", "Alpha"),
            ("Unknown", "Unclassified"),
        ];
        let groups = group_options(&items, |item| item.1.to_string(), "unclassified");
        assert_eq!(
            groups
                .into_iter()
                .map(|(group, _)| group)
                .collect::<Vec<_>>(),
            ["Alpha", "Beta", "Gamma", "Unclassified"],
        );
    }
}
