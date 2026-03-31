#[test]
fn derive_macro_reports_expected_failures() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/derive_fail_borrowed_fields.rs");
    tests.compile_fail("tests/ui/derive_fail_multiple_borrowed_fields.rs");
    tests.compile_fail("tests/ui/derive_fail_phantom_lifetime_without_skip.rs");
    tests.compile_fail("tests/ui/derive_fail_non_struct.rs");
    tests.compile_fail("tests/ui/derive_fail_enum_recall_nested.rs");
    tests.compile_fail("tests/ui/derive_fail_enum_recall_skip.rs");
    tests.compile_fail("tests/ui/derive_fail_enum_borrowed_fields.rs");
    tests.compile_fail("tests/ui/derive_fail_recallable_field_not_path.rs");
    tests.compile_fail("tests/ui/derive_fail_recallable_unknown_parameter.rs");
    tests.compile_fail("tests/ui/derive_fail_recallable_skip_with_unknown_parameter.rs");
    tests.compile_fail("tests/ui/derive_fail_recallable_name_value_parameter.rs");
    tests.compile_fail("tests/ui/derive_fail_recallable_conflicting_attributes.rs");
    tests.compile_fail("tests/ui/derive_fail_recallable_skip_on_struct.rs");
    tests.compile_fail("tests/ui/model_fail_recallable_skip_with_unknown_parameter.rs");
    tests.compile_fail("tests/ui/model_fail_recallable_conflicting_attributes.rs");
    tests.compile_fail("tests/ui/model_fail_enum_recallable_variant.rs");
    tests.compile_fail("tests/ui/model_fail_enum_skip_variant.rs");
    tests.pass("tests/ui/derive_pass_skip_memento_default_derives.rs");
    tests.pass("tests/ui/derive_pass_enum_no_warnings.rs");
    #[cfg(feature = "serde")]
    {
        tests.compile_fail("tests/ui/model_fail_duplicate_serialize.rs");
        tests.compile_fail("tests/ui/model_fail_duplicate_serialize_qualified.rs");
        tests.compile_fail("tests/ui/model_fail_duplicate_serialize_fully_qualified.rs");
        tests.compile_fail("tests/ui/derive_fail_serde_rename_conflict.rs");
        tests.compile_fail("tests/ui/derive_fail_serde_attr_on_skip.rs");
    }
}
