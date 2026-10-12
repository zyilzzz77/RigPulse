use crate::Application;
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Desktop,
    Store,
    Recent,
    Large,
    Unknown,
    System,
}
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Name,
    Publisher,
    Size,
    Installed,
}
pub fn select<'a>(
    apps: &'a [Application],
    search: &str,
    filter: Filter,
    sort: Sort,
) -> Vec<&'a Application> {
    let search = search.trim().to_lowercase();
    let cutoff = chrono::Utc::now().date_naive() - chrono::Duration::days(30);
    let mut result: Vec<_> = apps
        .iter()
        .filter(|a| {
            (filter == Filter::System || !a.protected)
                && (search.is_empty()
                    || a.name.to_lowercase().contains(&search)
                    || a.publisher
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&search))
                && match filter {
                    Filter::All => true,
                    Filter::System => a.protected,
                    Filter::Desktop => a.kind != crate::AppKind::Store,
                    Filter::Store => a.kind == crate::AppKind::Store,
                    Filter::Recent => a.installed.is_some_and(|d| d >= cutoff),
                    Filter::Large => a.estimated_bytes.is_some_and(|b| b >= 1024 * 1024 * 1024),
                    Filter::Unknown => a.estimated_bytes.is_none(),
                }
        })
        .collect();
    result.sort_by(|a, b| {
        match sort {
            Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            Sort::Publisher => match (&a.publisher, &b.publisher) {
                (Some(a), Some(b)) => a.to_lowercase().cmp(&b.to_lowercase()),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            },
            Sort::Size => b.estimated_bytes.cmp(&a.estimated_bytes),
            Sort::Installed => b.installed.cmp(&a.installed),
        }
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    result
}
