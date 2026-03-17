use recallable::{Recall, Recallable, TryRecall};

mod common;

use common::*;

#[test]
fn test_scoped_peek() -> anyhow::Result<()> {
    let fake_measurement: FakeMeasurement<i32, fn(&i32) -> i32> = FakeMeasurement {
        v: 42,
        how: identity,
    };
    let scoped_peek0 = ScopedMeasurement {
        current_control_level: 33u32,
        inner: fake_measurement.clone(),
        current_base: MeasurementResult(20i32),
    };
    let mut scoped_peek1 = ScopedMeasurement {
        current_control_level: 0u32,
        inner: fake_measurement.clone(),
        current_base: MeasurementResult(0i32),
    };
    let state0 = serde_json::to_string(&scoped_peek0)?;
    scoped_peek1.recall(serde_json::from_str(&state0)?);
    let state1 = serde_json::to_string(&scoped_peek0)?;
    assert!(state0 == state1);
    Ok(())
}

#[test]
fn test_scoped_measurement_memento_value_equality() {
    let measurement: FakeMeasurement<i32, fn(&i32) -> i32> = FakeMeasurement {
        v: 42,
        how: identity,
    };
    let original = ScopedMeasurement {
        current_control_level: 33u32,
        inner: measurement.clone(),
        current_base: MeasurementResult(20i32),
    };

    let json = serde_json::to_string(&original).unwrap();
    let memento: ScopedMeasurementMemento = serde_json::from_str(&json).unwrap();

    let mut target = ScopedMeasurement {
        current_control_level: 0u32,
        inner: measurement,
        current_base: MeasurementResult(0i32),
    };
    target.recall(memento);

    assert_eq!(target.current_control_level, 33);
    assert_eq!(target.inner.v, 42);
    assert_eq!(target.current_base, MeasurementResult(20));
}

#[test]
fn test_try_recall_blanket_impl() {
    let mut s = SimpleStruct { val: 10 };
    // The derived memento struct is compatible with serde.
    // We use from_str to create the memento value.
    let memento: <SimpleStruct as Recallable>::Memento =
        serde_json::from_str(r#"{"val": 20}"#).unwrap();

    // Should always succeed for `Recall` types due to the blanket impl.
    let result = s.try_recall(memento);
    assert!(result.is_ok());
    assert_eq!(s.val, 20);
}

#[test]
fn test_tuple_struct_memento() {
    let mut s = TupleStruct(1, 2);
    let memento: <TupleStruct as Recallable>::Memento =
        serde_json::from_str(r#"[10, 20]"#).unwrap();
    s.recall(memento);
    assert_eq!(s, TupleStruct(10, 20));
}

#[test]
fn test_tuple_struct_skip_keeps_original_field_index() {
    let mut s = TupleStructWithSkippedMiddle(1, identity, 2);
    let memento: TupleStructWithSkippedMiddleMemento = serde_json::from_str(r#"[10, 20]"#).unwrap();
    s.recall(memento);
    assert_eq!(s.0, 10);
    assert_eq!(s.2, 20);
}

#[test]
fn test_tuple_struct_with_where_clause() {
    let mut s = TupleStructWithWhereClause(1, (0, 0), 2);
    let memento: <TupleStructWithWhereClause<(u32, u32)> as Recallable>::Memento =
        serde_json::from_str(r#"[10, [42, 84], 20]"#).unwrap();
    s.recall(memento);
    assert_eq!(s.0, 10);
    assert_eq!(s.1, (42, 84));
    assert_eq!(s.2, 20);
}

#[test]
fn test_unit_struct_memento() {
    let mut s = UnitStruct;
    let memento: <UnitStruct as Recallable>::Memento = serde_json::from_str("null").unwrap();
    s.recall(memento);
    assert_eq!(s, UnitStruct);
}

#[test]
fn test_skip_serializing_field_is_excluded() {
    let mut s = SkipSerializingStruct {
        skipped: 5,
        value: 10,
    };
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json, serde_json::json!({ "value": 10 }));

    let memento: <SkipSerializingStruct as Recallable>::Memento =
        serde_json::from_str(r#"{"value": 42}"#).unwrap();
    s.recall(memento);
    assert_eq!(s.skipped, 5);
    assert_eq!(s.value, 42);
}

#[test]
fn test_direct_derive_does_not_add_serde_skip() {
    let value = DeriveOnlySkipBehavior {
        hidden: 7,
        shown: 11,
    };
    let json = serde_json::to_value(&value).unwrap();
    assert_eq!(json, serde_json::json!({ "hidden": 7, "shown": 11 }));

    let memento: <DeriveOnlySkipBehavior as Recallable>::Memento =
        serde_json::from_str(r#"{"shown": 5}"#).unwrap();
    let mut target = DeriveOnlySkipBehavior {
        hidden: 99,
        shown: 0,
    };
    target.recall(memento);

    assert_eq!(target.hidden, 99);
    assert_eq!(target.shown, 5);
}

#[test]
fn test_mixed_generic_usage_recalles_and_replaces() {
    let mut value = MixedGenericUsage {
        history: vec![Counter { value: 1 }],
        current: Counter { value: 2 },
    };
    let memento: <MixedGenericUsage<Counter, Vec<Counter>> as Recallable>::Memento =
        serde_json::from_str(r#"{"history":[{"value":10},{"value":20}],"current":{"value":99}}"#)
            .unwrap();

    value.recall(memento);
    assert_eq!(
        value.history,
        vec![Counter { value: 10 }, Counter { value: 20 }]
    );
    assert_eq!(value.current, Counter { value: 99 });
}

#[test]
fn test_existing_where_clause_with_trailing_comma() {
    let mut value = ExistingWhereTrailing {
        inner: Counter { value: 1 },
        marker: (),
    };
    let memento: <ExistingWhereTrailing<Counter, ()> as Recallable>::Memento =
        serde_json::from_str(r#"{"inner":{"value":5},"marker":null}"#).unwrap();

    value.recall(memento);
    assert_eq!(
        value,
        ExistingWhereTrailing {
            inner: Counter { value: 5 },
            marker: (),
        }
    );
}

#[test]
fn test_existing_where_clause_without_trailing_comma() {
    let mut value = ExistingWhereNoTrailing {
        inner: Counter { value: 3 },
    };
    let memento: <ExistingWhereNoTrailing<Counter> as Recallable>::Memento =
        serde_json::from_str(r#"{"inner":{"value":8}}"#).unwrap();

    value.recall(memento);
    assert_eq!(
        value,
        ExistingWhereNoTrailing {
            inner: Counter { value: 8 },
        }
    );
}
