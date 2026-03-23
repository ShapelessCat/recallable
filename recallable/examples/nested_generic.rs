use recallable::{Recall, Recallable};

#[derive(Clone, Debug, Recallable, Recall)]
struct InnerCounter {
    value: u32,
}

#[derive(Clone, Debug, Recallable, Recall)]
struct Envelope<T> {
    payload: T,
    #[recallable]
    inner: InnerCounter,
    #[recallable(skip)]
    cache_label: String,
}

fn main() {
    let mut envelope = Envelope {
        payload: "stale".to_string(),
        inner: InnerCounter { value: 0 },
        cache_label: "warm-cache".to_string(),
    };

    let memento: <Envelope<String> as Recallable>::Memento =
        serde_json::from_str(r#"{"payload":"fresh","inner":{"value":9}}"#).unwrap();

    envelope.recall(memento);

    assert_eq!(envelope.payload, "fresh");
    assert_eq!(envelope.inner.value, 9);
    assert_eq!(envelope.cache_label, "warm-cache");
}
