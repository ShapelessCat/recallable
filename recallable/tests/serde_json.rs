use proptest::{prelude::*, proptest};
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

#[test]
fn test_malformed_json_is_rejected() {
    // Reject obviously invalid syntax so arbitrary input cannot be mistaken for
    // a valid memento.
    let result = serde_json::from_str::<PropertyOuterMemento>("{ definitely not valid json");
    assert!(result.is_err());
}

#[test]
fn test_truncated_json_is_rejected() {
    // Truncation is a distinct corruption mode from malformed syntax and is
    // common when persisted output is cut off mid-write.
    let result = serde_json::from_str::<PropertyOuterMemento>(
        r#"{"level":7,"threshold":11,"nested":{"enabled":true,"lanes":[1,2"#,
    );
    assert!(result.is_err());
}

#[test]
fn test_json_schema_drift_added_field_is_accepted() {
    // Serde JSON ignores unknown fields by default. This test locks in that an
    // older reader can still accept a payload from a newer shape with extras.
    let payload = serde_json::to_string(&SchemaDriftAddedFieldV2 {
        id: 7,
        active: true,
        revision: 3,
    })
    .unwrap();

    let parsed: SchemaDriftV1 = serde_json::from_str(&payload).unwrap();
    assert_eq!(
        parsed,
        SchemaDriftV1 {
            id: 7,
            active: true,
        }
    );
}

#[test]
fn test_json_schema_drift_removed_field_is_rejected() {
    // Removing a required field is a breaking change for the old reader; this
    // should fail rather than silently invent a missing value.
    let payload = serde_json::to_string(&SchemaDriftRemovedFieldV2 { id: 7 }).unwrap();
    let result = serde_json::from_str::<SchemaDriftV1>(&payload);
    assert!(result.is_err());
}

#[test]
fn test_json_schema_drift_renamed_field_is_rejected() {
    // A rename is also breaking unless aliases are added explicitly, because
    // the old field name no longer appears in the payload.
    let payload = serde_json::to_string(&SchemaDriftRenamedFieldV2 {
        id: 7,
        is_active: true,
    })
    .unwrap();
    let result = serde_json::from_str::<SchemaDriftV1>(&payload);
    assert!(result.is_err());
}

#[test]
fn test_rename_field_round_trip() {
    let original = RenamedFields { level: 42, tag: 7 };
    let json = serde_json::to_string(&original).unwrap();

    // Serialized form uses the wire name
    assert!(json.contains("wire_level"));
    assert!(!json.contains("\"level\""));

    // Deserialize into memento using the wire name
    let memento: <RenamedFields as Recallable>::Memento = serde_json::from_str(&json).unwrap();

    let mut target = RenamedFields { level: 0, tag: 0 };
    target.recall(memento);
    assert_eq!(target.level, 42);
    assert_eq!(target.tag, 7);
}

#[test]
fn test_alias_field_deserialization() {
    // Deserialize using the alias key name
    let json = r#"{"old_level": 99, "tag": 3}"#;
    let memento: <AliasedFields as Recallable>::Memento = serde_json::from_str(json).unwrap();

    let mut target = AliasedFields { level: 0, tag: 0 };
    target.recall(memento);
    assert_eq!(target.level, 99);
    assert_eq!(target.tag, 3);

    // Also works with the other alias
    let json2 = r#"{"legacy_level": 77, "tag": 5}"#;
    let memento2: <AliasedFields as Recallable>::Memento = serde_json::from_str(json2).unwrap();

    let mut target2 = AliasedFields { level: 0, tag: 0 };
    target2.recall(memento2);
    assert_eq!(target2.level, 77);
    assert_eq!(target2.tag, 5);

    // And with the original field name
    let json3 = r#"{"level": 55, "tag": 1}"#;
    let memento3: <AliasedFields as Recallable>::Memento = serde_json::from_str(json3).unwrap();

    let mut target3 = AliasedFields { level: 0, tag: 0 };
    target3.recall(memento3);
    assert_eq!(target3.level, 55);
}

#[test]
fn test_rename_and_alias_combined() {
    let original = RenameAndAlias { level: 10, tag: 2 };
    let json = serde_json::to_string(&original).unwrap();

    // Serialized form uses the renamed key
    assert!(json.contains("wire_level"));

    // Deserialize with the renamed key
    let memento: <RenameAndAlias as Recallable>::Memento = serde_json::from_str(&json).unwrap();
    let mut target = RenameAndAlias { level: 0, tag: 0 };
    target.recall(memento);
    assert_eq!(target.level, 10);

    // Deserialize with the alias key
    let alias_json = r#"{"old_level": 20, "tag": 4}"#;
    let memento2: <RenameAndAlias as Recallable>::Memento =
        serde_json::from_str(alias_json).unwrap();
    let mut target2 = RenameAndAlias { level: 0, tag: 0 };
    target2.recall(memento2);
    assert_eq!(target2.level, 20);
}

#[test]
fn test_enum_variant_rename_round_trip() {
    let original = RenamedEnumFields::A { x: 42, y: 7 };
    let json = serde_json::to_string(&original).unwrap();

    // Serialized form uses the wire name
    assert!(json.contains("wire_x"));

    // Deserialize into memento
    let memento: <RenamedEnumFields as Recallable>::Memento = serde_json::from_str(&json).unwrap();

    let mut target = RenamedEnumFields::A { x: 0, y: 0 };
    target.recall(memento);
    assert_eq!(target, RenamedEnumFields::A { x: 42, y: 7 });
}

#[test]
fn test_enum_variant_alias_deserialization() {
    let json = r#"{"B":{"old_z":"hello"}}"#;
    let memento: <RenamedEnumFields as Recallable>::Memento = serde_json::from_str(json).unwrap();

    let mut target = RenamedEnumFields::B { z: String::new() };
    target.recall(memento);
    assert_eq!(target, RenamedEnumFields::B { z: "hello".into() });
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn property_json_roundtrip_recall_preserves_persisted_state(
        original in property_outer_strategy(),
        target_skipped_marker in any::<u8>(),
    ) {
        // Check three invariants over many shapes:
        // 1. persisted scalar fields survive serialize/deserialize/recall,
        // 2. nested #[recallable] fields round-trip as nested mementos,
        // 3. #[recallable(skip)] fields keep the target's existing value.
        let payload = serde_json::to_string(&original).unwrap();
        let memento: PropertyOuterMemento = serde_json::from_str(&payload).unwrap();
        let inspectable: InspectablePropertyOuterMemento = serde_json::from_str(&payload).unwrap();
        let expected_nested: PropertyInnerMemento =
            serde_json::from_str(&serde_json::to_string(&original.nested).unwrap()).unwrap();

        prop_assert_eq!(inspectable.nested, expected_nested);

        let mut target = property_seed(target_skipped_marker);
        target.recall(memento);

        prop_assert_eq!(target.level, original.level);
        prop_assert_eq!(target.threshold, original.threshold);
        prop_assert_eq!(target.nested, original.nested);
        prop_assert_eq!(target.skipped_marker, target_skipped_marker);
    }
}
