use rigpulse_apps::{
    Application,
    query::{Filter, Sort, select},
};
#[test]
fn search_is_case_insensitive_and_unknown_sizes_sort_last() {
    let apps = vec![
        Application {
            name: "Editor".into(),
            publisher: Some("ACME".into()),
            estimated_bytes: None,
            ..Default::default()
        },
        Application {
            name: "Editor Large".into(),
            publisher: Some("Acme".into()),
            estimated_bytes: Some(2_000_000_000),
            ..Default::default()
        },
        Application {
            name: "System".into(),
            publisher: Some("ACME".into()),
            protected: true,
            ..Default::default()
        },
    ];
    let found = select(&apps, "acME", Filter::All, Sort::Size);
    assert_eq!(
        found.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["Editor Large", "Editor"]
    );
    assert_eq!(select(&apps, "", Filter::System, Sort::Name).len(), 1);
    assert_eq!(select(&apps, "missing", Filter::All, Sort::Name).len(), 0);
}
