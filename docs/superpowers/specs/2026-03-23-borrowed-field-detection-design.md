# Borrowed Field Detection — Design Spec

## Problem

The current `validate_generics` function blanket-rejects any struct with lifetime
parameters. This is overly broad — structs like `Foo<'a>` with only
`PhantomData<&'a ()>` fields contain no actual borrowed data and should be allowed.

## Goal

Replace the struct-level lifetime rejection with field-level borrowed field
detection. Only reject fields where a struct lifetime parameter actually appears
in the field's type, exempting `PhantomData` and `#[recallable(skip)]` fields.

## Design

### Validation Logic

Replace `validate_generics` with a new validation function. In
`MacroContext::new`, this runs after `extract_struct_fields` but before
`collect_field_actions`, since it needs access to the parsed fields.

1. Collect the set of lifetime parameter idents from `input.generics.params`.
   Note: `syn::Lifetime` stores idents *without* the tick — `'a` becomes `Ident("a")`.
2. If the set is empty, return `Ok(())` immediately.
3. For each field (named or unnamed — applies to both regular and tuple structs):
   a. Call `has_recallable_skip_attr(field)` (the existing public helper that
      delegates to `determine_field_behavior`). If the field is skipped, skip
      the borrow check — skipped fields don't appear in the memento, so
      borrowed data there is harmless. Attribute validation errors are left to
      `collect_field_actions` which runs later.
   b. If the field's outermost type is `PhantomData` (last segment of a
      `TypePath` is `PhantomData`), skip it.
   c. Walk the field's `syn::Type` using a `syn::visit::Visit` impl that looks
      for `Lifetime` nodes whose ident matches any collected struct lifetime.
   d. If a match is found, emit `syn::Error` on the field's type span.
4. Collect all errors and combine them (so the user sees every offending field).

### Lifetime Visitor

A new `LifetimeUsageChecker` struct implementing `Visit`:

```rust
struct LifetimeUsageChecker<'a> {
    struct_lifetimes: &'a HashSet<&'a Ident>,
    found: bool,
}

impl<'ast> Visit<'ast> for LifetimeUsageChecker<'_> {
    fn visit_lifetime(&mut self, lt: &'ast Lifetime) {
        if self.struct_lifetimes.contains(&lt.ident) {
            self.found = true;
        }
    }
}
```

The visitor does not short-circuit after `found = true` — this is intentional
for simplicity. The type trees are small and the cost is negligible.

This is a separate visitor from the existing `SimpleTypeCollector` — they serve
different purposes (ident collection vs. lifetime presence check) and share no
logic worth abstracting.

### PhantomData Detection

A field is `PhantomData` if its type is a `TypePath` whose last path segment
ident is `PhantomData`. This handles `PhantomData<...>`,
`core::marker::PhantomData<...>`, and `std::marker::PhantomData<...>`.

### Generated Code for Passing Structs

When a lifetime-parameterized struct passes validation (e.g., only PhantomData
uses the lifetime), the generated code works correctly because:

- The memento struct omits lifetime parameters — it only includes type params
  (existing behavior via `generics.type_params()`).
- The trait impls use `split_for_impl()` which naturally includes the lifetime:
  `impl<'a> Recallable for Foo<'a> { type Memento = FooMemento; }` — valid
  because `FooMemento` simply doesn't reference `'a`.

### Error Message

```
Recall derives do not support borrowed fields
```

Error span points at the offending field's type, not the struct's generics.

### What Passes (Previously Rejected)

```rust
#[derive(Recallable)]
struct Foo<'a> {
    marker: PhantomData<&'a ()>,
    value: u8,
}
```

### What Still Fails

```rust
// Direct reference
#[derive(Recallable)]
struct Bar<'a> {
    name: &'a str,           // error: borrowed field
}

// Nested reference
#[derive(Recallable)]
struct Baz<'a> {
    data: Vec<&'a u8>,       // error: borrowed field
}

// Multiple errors reported at once
#[derive(Recallable)]
struct Multi<'a> {
    a: &'a str,              // error
    b: Option<&'a u8>,       // error (both reported)
    marker: PhantomData<&'a ()>,  // ok
}

// Tuple struct
#[derive(Recallable)]
struct Wrapper<'a>(&'a str);  // error: borrowed field

// Multiple lifetime parameters
#[derive(Recallable)]
struct Multi2<'a, 'b> {
    x: &'a str,              // error
    y: &'b str,              // error (both lifetimes checked)
    marker: PhantomData<&'a ()>,  // ok
}
```

### Skipped Fields with Borrows

Skipped fields are exempt from the borrow check. They don't appear in the
memento, so borrowed data there is harmless:

```rust
#[derive(Recallable)]
struct WithSkipped<'a> {
    #[recallable(skip)]
    name: &'a str,           // ok — skipped from memento
    value: u8,
}
```

## Files Changed

| File | Change |
|------|--------|
| `recallable-macro/src/context.rs` | Replace `validate_generics` with field-level validation + `LifetimeUsageChecker` visitor |
| `recallable/tests/ui/derive_fail_lifetime_parameterized_struct.rs` | Rename → `derive_fail_borrowed_fields.rs`, use actual borrowed field |
| `recallable/tests/ui/derive_fail_lifetime_parameterized_struct.stderr` | Rename → `derive_fail_borrowed_fields.stderr`, update span |
| `recallable/tests/macro_expansion_failures.rs` | Update test path back to `derive_fail_borrowed_fields.rs` |
| `recallable/tests/` (new) | Add passing test for PhantomData-only lifetime struct |

## Out of Scope

- Rejecting `'static` lifetimes in field types (these are not struct lifetime params)
- Lifetime elision or inference
