use recallable::{Recall, Recallable, recallable_model};

#[recallable_model]
#[derive(Clone, Debug, PartialEq)]
struct DashboardState {
    volume: u8,
    label: String,
    #[recallable(skip)]
    cache_key: String,
}

fn main() {
    let mut dashboard = DashboardState {
        volume: 10,
        label: "draft".to_string(),
        cache_key: "keep-me".to_string(),
    };

    let memento: <DashboardState as Recallable>::Memento =
        serde_json::from_str(r#"{"volume":75,"label":"live"}"#).unwrap();

    dashboard.recall(memento);

    assert_eq!(dashboard.volume, 75);
    assert_eq!(dashboard.label, "live");
    assert_eq!(dashboard.cache_key, "keep-me");
}
