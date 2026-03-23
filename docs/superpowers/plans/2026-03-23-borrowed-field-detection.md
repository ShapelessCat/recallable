# Borrowed Field Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the blanket lifetime-parameter rejection with field-level borrowed field detection, exempting `PhantomData` and `#[recallable(skip)]` fields. Auto-skip `PhantomData` fields that use struct lifetimes from the memento.

**Architecture:** Add a `LifetimeUsageChecker` visitor and `is_phantom_data` helper as free functions in `context.rs`. Replace `validate_generics` with `validate_no_borrowed_fields` (associated function, runs after field extraction). Modify `collect_field_action` to auto-skip PhantomData fields that reference struct lifetimes. Update UI tests to match the new per-field error behavior.

**Tech Stack:** Rust, `syn` (visit, types), `trybuild` (compile-fail tests)

**Spec:** `docs/superpowers/specs/2026-03-23-borrowed-field-detection-design.md`

---

### Task 1: Add `LifetimeUsageChecker` visitor and `is_phantom_data` helper

**Files:**
- Modify: `recallable-macro/src/context.rs` (add after `is_generic_type_param` ending at line 412)

- [ ] **Step 1: Add `HashSet` to the imports**

Change line 16 from:

```rust
use std::collections::HashMap;
```

To:

```rust
use std::collections::{HashMap, HashSet};
```

- [ ] **Step 2: Update existing `is_generic_type_param` to use unqualified `HashSet`**

Change line 402 from:

```rust
    generic_type_params: &std::collections::HashSet<&Ident>,
```

To:

```rust
    generic_type_params: &HashSet<&Ident>,
```

- [ ] **Step 3: Add the `LifetimeUsageChecker` struct and `Visit` impl**

Add after the `is_generic_type_param` function (line 412):

```rust
struct LifetimeUsageChecker<'a> {
    struct_lifetimes: &'a HashSet<&'a Ident>,
    found: bool,
}

impl<'ast> Visit<'ast> for LifetimeUsageChecker<'_> {
    fn visit_lifetime(&mut self, lt: &'ast syn::Lifetime) {
        if self.struct_lifetimes.contains(&lt.ident) {
            self.found = true;
        }
    }
}
```

- [ ] **Step 4: Add the `is_phantom_data` helper function**

Add after `LifetimeUsageChecker`:

```rust
fn is_phantom_data(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "PhantomData")
    } else {
        false
    }
}
```

- [ ] **Step 5: Add a `field_uses_struct_lifetime` helper**

Add after `is_phantom_data`:

```rust
fn field_uses_struct_lifetime(
    ty: &Type,
    struct_lifetimes: &HashSet<&Ident>,
) -> bool {
    let mut checker = LifetimeUsageChecker {
        struct_lifetimes,
        found: false,
    };
    checker.visit_type(ty);
    checker.found
}
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo build --package recallable-macro`
Expected: compiles (new code is unused for now)

- [ ] **Step 7: Commit**

```bash
git add recallable-macro/src/context.rs
git commit -m "feat: add LifetimeUsageChecker visitor, is_phantom_data, and field_uses_struct_lifetime helpers"
```

---

### Task 2: Replace `validate_generics` with `validate_no_borrowed_fields`

**Files:**
- Modify: `recallable-macro/src/context.rs:68-104` (the `new` constructor and `validate_generics`)

- [ ] **Step 1: Replace `validate_generics` with `validate_no_borrowed_fields`**

Replace the `validate_generics` associated function (lines 90-104 inside `impl<'a> MacroContext<'a>`) in-place with:

```rust
fn validate_no_borrowed_fields(input: &DeriveInput, fields: &Fields) -> syn::Result<()> {
    let struct_lifetimes: HashSet<&Ident> = input
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            GenericParam::Lifetime(lt) => Some(&lt.lifetime.ident),
            _ => None,
        })
        .collect();

    if struct_lifetimes.is_empty() {
        return Ok(());
    }

    let mut errors: Option<syn::Error> = None;

    for field in fields.iter() {
        if has_recallable_skip_attr(field) {
            continue;
        }
        if is_phantom_data(&field.ty) {
            continue;
        }
        if field_uses_struct_lifetime(&field.ty, &struct_lifetimes) {
            let err = syn::Error::new_spanned(
                &field.ty,
                "Recall derives do not support borrowed fields",
            );
            match &mut errors {
                Some(existing) => existing.combine(err),
                None => errors = Some(err),
            }
        }
    }

    match errors {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
```

- [ ] **Step 2: Update `MacroContext::new` call order**

Change lines 68-70 from:

```rust
pub(crate) fn new(input: &'a DeriveInput) -> syn::Result<Self> {
    Self::validate_generics(input)?;
    let fields = Self::extract_struct_fields(input)?;
```

To:

```rust
pub(crate) fn new(input: &'a DeriveInput) -> syn::Result<Self> {
    let fields = Self::extract_struct_fields(input)?;
    Self::validate_no_borrowed_fields(input, fields)?;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build --package recallable-macro`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
git add recallable-macro/src/context.rs
git commit -m "feat: replace validate_generics with field-level validate_no_borrowed_fields"
```

---

### Task 3: Auto-skip PhantomData fields with struct lifetimes from memento

**Files:**
- Modify: `recallable-macro/src/context.rs:117-159` (`collect_field_actions` / `collect_field_action`)

The `MacroContext` needs to know the struct's lifetime params so `collect_field_action` can auto-skip PhantomData fields that reference them. The simplest approach: compute the `HashSet<&Ident>` of struct lifetime idents in `collect_field_actions` and pass it down.

- [ ] **Step 1: Update `collect_field_actions` to compute struct lifetimes and pass them**

Change `collect_field_actions` (lines 117-128) to accept `generics: &'a Generics`:

```rust
fn collect_field_actions(
    fields: &'a Fields,
    generics: &'a Generics,
) -> syn::Result<(HashMap<&'a Ident, TypeUsage>, Vec<FieldAction<'a>>)> {
    let struct_lifetimes: HashSet<&Ident> = generics
        .params
        .iter()
        .filter_map(|p| match p {
            GenericParam::Lifetime(lt) => Some(&lt.lifetime.ident),
            _ => None,
        })
        .collect();

    let mut preserved_types = HashMap::new();
    let mut field_actions = Vec::with_capacity(fields.len());

    for (index, field) in fields.iter().enumerate() {
        Self::collect_field_action(
            index,
            field,
            &struct_lifetimes,
            &mut preserved_types,
            &mut field_actions,
        )?;
    }

    Ok((preserved_types, field_actions))
}
```

- [ ] **Step 2: Update `collect_field_action` to auto-skip PhantomData with struct lifetimes**

Change `collect_field_action` (lines 130-158) to accept the lifetime set and skip PhantomData fields that use struct lifetimes:

```rust
fn collect_field_action(
    index: usize,
    field: &'a Field,
    struct_lifetimes: &HashSet<&Ident>,
    preserved_types: &mut HashMap<&'a Ident, TypeUsage>,
    field_actions: &mut Vec<FieldAction<'a>>,
) -> syn::Result<()> {
    if is_phantom_data(&field.ty) && field_uses_struct_lifetime(&field.ty, struct_lifetimes) {
        // Auto-skip: PhantomData fields referencing struct lifetimes cannot
        // appear in the memento (which omits lifetime parameters).
        return Ok(());
    }
    if let Some(field_behavior) = Self::determine_field_behavior(field)? {
        let member = Self::field_member(field, index);
        let field_type = &field.ty;
        match field_behavior {
            FieldBehavior::Recall => {
                if let Some(type_name) = Self::extract_recallable_type_name(field_type)? {
                    preserved_types.insert(type_name, TypeUsage::Recallable);
                }
            }
            FieldBehavior::Keep => {
                Self::record_non_recallable_type_usage(field_type, preserved_types);
            }
        }
        field_actions.push(FieldAction {
            member,
            ty: field_type,
            behavior: field_behavior,
        });
    }
    Ok(())
}
```

- [ ] **Step 3: Update the call site in `MacroContext::new`**

Change line 71 from:

```rust
let (preserved_types, field_actions) = Self::collect_field_actions(fields)?;
```

To:

```rust
let (preserved_types, field_actions) = Self::collect_field_actions(fields, &input.generics)?;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build --package recallable-macro`
Expected: compiles successfully

- [ ] **Step 5: Commit**

```bash
git add recallable-macro/src/context.rs
git commit -m "feat: auto-skip PhantomData fields with struct lifetimes from memento"
```

---

### Task 4: Update the compile-fail UI test

**Files:**
- Create: `recallable/tests/ui/derive_fail_borrowed_fields.rs`
- Create: `recallable/tests/ui/derive_fail_borrowed_fields.stderr`
- Create: `recallable/tests/ui/derive_fail_multiple_borrowed_fields.rs`
- Create: `recallable/tests/ui/derive_fail_multiple_borrowed_fields.stderr`
- Delete: `recallable/tests/ui/derive_fail_lifetime_parameterized_struct.rs`
- Delete: `recallable/tests/ui/derive_fail_lifetime_parameterized_struct.stderr`
- Modify: `recallable/tests/macro_expansion_failures.rs:4`

- [ ] **Step 1: Create the single-field compile-fail test**

Write `recallable/tests/ui/derive_fail_borrowed_fields.rs`:

```rust
use recallable::Recallable;

#[derive(Recallable)]
struct BorrowedValue<'a> {
    value: &'a str,
}

fn main() {}
```

- [ ] **Step 2: Create the multi-field compile-fail test**

This validates that multiple borrowed fields each produce their own error (error-combining logic).

Write `recallable/tests/ui/derive_fail_multiple_borrowed_fields.rs`:

```rust
use core::marker::PhantomData;
use recallable::Recallable;

#[derive(Recallable)]
struct MultiBorrowed<'a> {
    a: &'a str,
    b: Vec<&'a u8>,
    marker: PhantomData<&'a ()>,
}

fn main() {}
```

- [ ] **Step 3: Update `macro_expansion_failures.rs`**

Change line 4 from:

```rust
    tests.compile_fail("tests/ui/derive_fail_lifetime_parameterized_struct.rs");
```

To:

```rust
    tests.compile_fail("tests/ui/derive_fail_borrowed_fields.rs");
    tests.compile_fail("tests/ui/derive_fail_multiple_borrowed_fields.rs");
```

- [ ] **Step 4: Delete the old test files**

```bash
rm recallable/tests/ui/derive_fail_lifetime_parameterized_struct.rs
rm recallable/tests/ui/derive_fail_lifetime_parameterized_struct.stderr
```

- [ ] **Step 5: Generate the `.stderr` files**

Use `TRYBUILD=overwrite` to capture the exact compiler output:

```bash
TRYBUILD=overwrite cargo test --package recallable --test macro_expansion_failures
```

Verify the generated `.stderr` files contain the expected errors:
- `derive_fail_borrowed_fields.stderr`: one error on `&'a str`
- `derive_fail_multiple_borrowed_fields.stderr`: two errors — one on `&'a str`, one on `Vec<&'a u8>`. The `PhantomData` field should NOT produce an error.

- [ ] **Step 6: Run the compile-fail tests**

Run: `cargo test --package recallable --test macro_expansion_failures`
Expected: all compile-fail tests pass

- [ ] **Step 7: Commit**

```bash
git add -A recallable/tests/
git commit -m "test: update compile-fail tests for borrowed field detection"
```

---

### Task 5: Add passing tests for PhantomData-only lifetime struct and skipped borrowed fields

**Files:**
- Modify: `recallable/tests/basic.rs`

- [ ] **Step 1: Write the PhantomData-only lifetime test**

Add to `recallable/tests/basic.rs`:

```rust
mod phantom_lifetime {
    use core::marker::PhantomData;
    use recallable::{Recall, Recallable};

    #[derive(Recallable, Recall)]
    struct PhantomLifetime<'a> {
        marker: PhantomData<&'a ()>,
        value: u8,
    }

    #[test]
    fn test_phantom_lifetime_recall() {
        let mut s = PhantomLifetime {
            marker: PhantomData,
            value: 10,
        };
        // marker is auto-skipped from memento (PhantomData with struct lifetime)
        let memento = PhantomLifetimeMemento { value: 42 };
        s.recall(memento);
        assert_eq!(s.value, 42);
    }
}
```

- [ ] **Step 2: Write the skipped borrowed field test**

Add to `recallable/tests/basic.rs`:

```rust
mod skipped_borrowed_field {
    use recallable::{Recall, Recallable};

    #[derive(Recallable, Recall)]
    struct WithSkippedBorrow<'a> {
        #[recallable(skip)]
        name: &'a str,
        value: u8,
    }

    #[test]
    fn test_skipped_borrowed_field_recall() {
        let mut s = WithSkippedBorrow {
            name: "hello",
            value: 10,
        };
        let memento = WithSkippedBorrowMemento { value: 42 };
        s.recall(memento);
        assert_eq!(s.value, 42);
        assert_eq!(s.name, "hello"); // unchanged — skipped
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --package recallable --test basic phantom_lifetime skipped_borrowed_field`
Expected: both tests PASS

- [ ] **Step 4: Commit**

```bash
git add recallable/tests/basic.rs
git commit -m "test: add passing tests for PhantomData lifetime and skipped borrowed fields"
```

---

### Task 6: Run full test suite and lint

- [ ] **Step 1: Run all tests with default features**

Run: `cargo test --package recallable`
Expected: all tests pass

- [ ] **Step 2: Run tests without serde**

Run: `cargo test --package recallable --no-default-features`
Expected: all tests pass

- [ ] **Step 3: Run tests with impl_from**

Run: `cargo test --package recallable --features impl_from`
Expected: all tests pass

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no warnings

- [ ] **Step 5: Run fmt check**

Run: `cargo fmt -- --check`
Expected: no formatting issues

- [ ] **Step 6: Commit any fixes if needed**

If any lint or format issues were found, fix and commit:

```bash
cargo fmt
git add -A
git commit -m "chore: fix formatting"
```
