# Borrowed Field Detection — Design Spec

## Problem

The current `validate_generics` function blanket-rejects any struct with lifetime
parameters. This is overly broad — structs like `Foo<'a>` with only
`PhantomData<&'a ()>` fields contain no actual borrowed data and should be allowed.

## Goal

Replace the struct-level lifetime rejection with field-level borrowed field
detection. Only reject fields where a struct lifetime parameter actually appears
in the field's type, exempting `PhantomData` fields.

## Design

### Validation Logic

Replace `validate_generics` with a new validation function (called from
`MacroContext::new`):

1. Collect the set of lifetime parameter idents from `input.generics.params`
   (e.g., `'a`, `'b`).
2. If the set is empty, return `Ok(())` immediately.
3. For each named field in the struct:
   a. Check if the field's outermost type is `PhantomData` (last segment of a
      `TypePath` is `PhantomData`). If so, skip it.
   b. Walk the field's `syn::Type` using a `syn::visit::Visit` impl that looks
      for `Lifetime` nodes whose ident matches any collected struct lifetime.
   c. If a match is found, emit `syn::Error` on the field's type span.
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

This is a separate visitor from the existing `SimpleTypeCollector` — they serve
different purposes (ident collection vs. lifetime presence check) and share no
logic worth abstracting.

### PhantomData Detection

A field is `PhantomData` if its type is a `TypePath` whose last path segment
ident is `PhantomData`. This handles `PhantomData<...>`,
`core::marker::PhantomData<...>`, and `std::marker::PhantomData<...>`.

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

### What Fails

```rust
#[derive(Recallable)]
struct Bar<'a> {
    name: &'a str,           // error: borrowed field
}

#[derive(Recallable)]
struct Baz<'a> {
    data: Vec<&'a u8>,       // error: nested borrowed field
}

#[derive(Recallable)]
struct Multi<'a> {
    a: &'a str,              // error
    b: Option<&'a u8>,       // error (both reported)
    marker: PhantomData<&'a ()>,  // ok
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
- Changing how `#[recallable(skip)]` interacts with borrowed fields
- Lifetime elision or inference
