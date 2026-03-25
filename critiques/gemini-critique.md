# Recallable Codebase Critique & Evaluation

Based on a comprehensive review of the `recallable` and `recallable-macro` codebase, here is an evaluation of its architectural design, code quality, and idiomatic correctness.

## What is Good

### 1. Excellent Macro Architecture (Analysis vs. Codegen)

The refactoring of the monolithic `MacroContext` into `StructIr` and specific code generation modules (`memento_struct.rs`, `recallable_impl.rs`, `recall_impl.rs`, etc.) is exactly how modern procedural macros should be structured in Rust.

* **Separation of Concerns:** `StructIr` purely handles semantic analysis (validating lifetimes, resolving generics, inferring field constraints) without being polluted by `quote!` tokens.
* **Testability:** This separation makes it far easier to unit-test the IR generation and the code emission independently.

### 2. Robust Generic Parameter Handling

The crate handles Rust's notoriously difficult generic constraints very well:

* By analyzing which generic parameters are retained vs. dropped (`GenericParamRetention`), the macro correctly emits synthetic `PhantomData` marker fields. This avoids the common macro pitfall where unused type parameters on the generated struct cause compile errors.
* Dynamic trait bound inference (e.g., adding `<T as Recallable>::Memento: Clone`) in `MementoTraitSpec` and `whole_type_bounds` ensures the generated code compiles even for complex nested generics.

### 3. Cargo Feature Toggles and Hygiene

* **Serde Integration:** The `serde` and `impl_from` features are elegantly integrated. `serde` injection happens deliberately, keeping the boilerplate low for the consumer.
* **Crate Resolution:** Using `proc_macro_crate` to resolve the `recallable` path ensures the macro works correctly even if the user renames the dependency in `Cargo.toml`. Preferring absolute paths (`::recallable` over `crate::`) ensures doctests remain functional.

### 4. Zero-Cost Abstractions and Documentation

* The `Recallable`, `Recall`, and `TryRecall` trait designs are idiomatic (similar to how Serde structures serializers).
* The inline documentation is stellar, complete with extensive, correct doctests and explanations of motivation.

---

## What is Bad / Areas for Improvement

### 1. Hardcoded Serde Assumptions

Currently, `recallable_model` heavily hardcodes Serde (`serde::Serialize`, `serde::Deserialize`, `#[serde(skip)]`). While Serde is ubiquitous, burning it directly into the macro's core limits flexibility:

* **Alternative Formats:** If a user wants to use `borsh`, `bincode` (without Serde), or `scale` (from Parity), the macro provides no assistance.
* **Serde Attribute Collisions:** `add_serde_skip_attrs` blindly injects `#[serde(skip)]`. If the user has heavily customized their serialization (e.g., using `#[serde(serialize_with = "...")]` on a computed field they still want to skip recalling), this might conflict.
* **Recommendation:** Introduce attribute pass-through. Let the user specify `#[recallable(derive(BorshSerialize, BorshDeserialize))]` so the macro can forward arbitrary derives to the generated memento struct instead of hardcoding `serde`.

### 2. Limited Field Customization (No "With" Attribute)

The macro currently only supports `#[recallable]` and `#[recallable(skip)]`. This breaks down when a user needs a specific field to be recalled, but that field belongs to a 3rd party crate (e.g., a trait object or custom collection) that doesn't implement `Recallable`.

* **Recommendation:** Implement `#[recallable(with = "module::path")]` or `#[recallable(memento_type = "MyType", recall_with = "my_fn")]`. This mirrors Serde's `#[serde(with)]` and allows users to define custom memento logic for foreign types without needing newtype wrappers.
