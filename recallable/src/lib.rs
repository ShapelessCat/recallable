//! # Recallable
//!
//! A crate for handling partial updates to data structures.
//!
//! This crate provides the [`Recallable`], [`Recall`], and [`TryRecall`] traits, along with
//! derive macros for `Recallable` and `Recall`, and an attribute macro `recallable_model`
//! re-exported from `recallable_macro` for easy derivation.
//!
//! ## Motivation
//!
//! Many systems receive incremental updates where only a subset of fields change or can be
//! considered part of the state. This crate formalizes this pattern by defining a memento type for
//! a structure and providing a consistent way to apply such mementos safely.

// Re-export the derive macros.
#![no_std]

extern crate self as recallable;

/// Attribute macro that prepares a struct for the Memento pattern.
///
/// Adds `#[derive(Recallable, Recall)]` automatically. When the `serde` feature is enabled,
/// also derives `serde::Serialize` on the struct and injects `#[serde(skip)]` on fields
/// marked with `#[recallable(skip)]`.
///
/// This example requires the `serde` feature.
///
/// ```rust
/// # #[cfg(feature = "serde")]
/// # {
/// use recallable::{Recall, Recallable, recallable_model};
///
/// #[recallable_model]
/// #[derive(Clone, Debug)]
/// struct Settings {
///     volume: u8,
///     brightness: u8,
///     #[recallable(skip)]
///     on_change: fn(),
/// }
///
/// fn noop() {}
///
/// let mut settings = Settings { volume: 50, brightness: 80, on_change: noop };
/// let memento: <Settings as Recallable>::Memento =
///     serde_json::from_str(r#"{"volume":75,"brightness":60}"#).unwrap();
/// settings.recall(memento);
/// assert_eq!(settings.volume, 75);
/// assert_eq!(settings.brightness, 60);
/// // on_change is skipped — unchanged by recall
/// # }
/// ```
pub use recallable_macro::recallable_model;

/// Derive macro that generates a companion memento struct and the [`Recallable`] trait impl.
///
/// The memento struct mirrors the original but replaces `#[recallable]`-annotated fields
/// with their `<FieldType as Recallable>::Memento` type and omits `#[recallable(skip)]` fields.
///
/// This example requires the `serde` feature.
///
/// ```rust
/// # #[cfg(feature = "serde")]
/// # {
/// use recallable::{Recall, Recallable};
///
/// #[derive(Clone, Debug, serde::Serialize, Recallable, Recall)]
/// struct Outer {
///     label: String,
///     #[recallable]
///     inner: Inner,
/// }
///
/// #[derive(Clone, Debug, serde::Serialize, Recallable, Recall)]
/// struct Inner {
///     count: u32,
/// }
///
/// // The memento type is accessible via the associated type
/// let memento: <Outer as Recallable>::Memento =
///     serde_json::from_str(r#"{"label":"updated","inner":{"count":99}}"#).unwrap();
///
/// let mut outer = Outer { label: "original".into(), inner: Inner { count: 0 } };
/// outer.recall(memento);
/// assert_eq!(outer.label, "updated");
/// assert_eq!(outer.inner.count, 99);
/// # }
/// ```
pub use recallable_macro::Recallable;

/// Derive macro that generates the [`Recall`] trait implementation.
///
/// For plain fields, `recall` assigns the memento value directly. For fields annotated
/// with `#[recallable]`, it recursively calls `recall` on the nested value.
/// Fields marked `#[recallable(skip)]` are left untouched.
///
/// This example requires the `serde` feature.
///
/// ```rust
/// # #[cfg(feature = "serde")]
/// # {
/// use recallable::{Recall, Recallable};
///
/// #[derive(Clone, Debug, serde::Serialize, Recallable, Recall)]
/// struct State {
///     score: i32,
///     #[recallable(skip)]
///     cached_label: String,
/// }
///
/// let mut state = State { score: 0, cached_label: "stale".into() };
/// let memento: <State as Recallable>::Memento =
///     serde_json::from_str(r#"{"score":42}"#).unwrap();
/// state.recall(memento);
/// assert_eq!(state.score, 42);
/// assert_eq!(state.cached_label, "stale"); // skip preserves the value
/// # }
/// ```
pub use recallable_macro::Recall;

/// A type that declares a companion memento type.
///
/// ## Usage
///
/// ```rust
/// use recallable::{Recall, Recallable};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize)]
/// pub struct Accumulator<T> {
///     prev_control_signal: T,
///     #[serde(skip)]
///     filter: fn(&i32) -> bool,
///     accumulated: u32,
/// }
///
/// //vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv
/// // If we derive `Recallable` and `Recall` for `Accumulator`, the equivalent companion memento
/// // type and the `Recallable`/`Recall` implementations can be generated automatically.
/// // The generated companion type is exposed as
/// // `<Accumulator<T> as Recallable>::Memento`; its concrete struct name is an implementation
/// // detail of the derive.
/// //
/// // When deriving `Recallable`, a `From<Accumulator>` implementation is generated if the
/// // `impl_from` feature is enabled. For derived implementations, mark non-state fields with
/// // `#[recallable(skip)]` (and add `#[serde(skip)]` as needed when using serde).
///
/// #[derive(PartialEq, Deserialize)]
/// pub struct AccumulatorMemento<T> {
///     prev_control_signal: T,
///     accumulated: u32,
/// }
///
/// impl<T> Recallable for Accumulator<T> {
///     type Memento = AccumulatorMemento<T>;
/// }
///
/// impl<T> From<Accumulator<T>> for AccumulatorMemento<T> {
///     fn from(acc: Accumulator<T>) -> Self {
///         Self {
///             prev_control_signal: acc.prev_control_signal,
///             accumulated: acc.accumulated,
///         }
///     }
/// }
///
/// impl<T> Recall for Accumulator<T> {
///     #[inline(always)]
///     fn recall(&mut self, memento: Self::Memento) {
///         self.prev_control_signal = memento.prev_control_signal;
///         self.accumulated = memento.accumulated;
///     }
/// }
/// //^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
///
/// fn main() {
///     let accumulator = Accumulator {
///         prev_control_signal: 6,
///         filter: |x: &i32| *x > 300,
///         accumulated: 15,
///     };
///
///     let state_bytes = postcard::to_vec::<_, 128>(&accumulator).unwrap();
///     let accumulator_memento: <Accumulator<i32> as Recallable>::Memento =
///         postcard::from_bytes(&state_bytes).unwrap();
///
///     let mut recovered_accumulator = Accumulator {
///         prev_control_signal: -1,
///         accumulated: 0,
///         ..accumulator
///     };
///
///     recovered_accumulator.recall(accumulator_memento);
///
///     assert_eq!(recovered_accumulator.prev_control_signal, accumulator.prev_control_signal);
///     assert_eq!(recovered_accumulator.accumulated, accumulator.accumulated);
/// }
/// ```
/// Declares the associated memento type.
pub trait Recallable {
    /// The type of memento associated with this structure.
    type Memento;
}

/// A type that can change state by absorbing one companion memento value.
///
/// # Example
///
/// ```rust
/// use recallable::{Recall, Recallable};
///
/// struct Settings {
///     volume: u32,
///     brightness: u32,
/// }
///
/// #[derive(Clone, Debug, PartialEq)]
/// struct SettingsMemento {
///     volume: u32,
///     brightness: u32,
/// }
///
/// impl Recallable for Settings {
///     type Memento = SettingsMemento;
/// }
///
/// impl Recall for Settings {
///     fn recall(&mut self, memento: Self::Memento) {
///         self.volume = memento.volume;
///         self.brightness = memento.brightness;
///     }
/// }
///
/// fn main() {
///    let mut settings = Settings { volume: 50, brightness: 70 };
///    let memento = SettingsMemento { volume: 80, brightness: 40 };
///    settings.recall(memento);
///    assert_eq!(settings.volume, 80);
///    assert_eq!(settings.brightness, 40);
/// }
/// ```
pub trait Recall: Recallable {
    /// Applies the given memento to update the structure.
    fn recall(&mut self, memento: Self::Memento);
}

/// A fallible variant of [`Recall`].
///
/// This trait lets you apply a memento with validation and return a custom error
/// if it cannot be applied.
///
/// ## Usage
///
/// ```rust
/// use recallable::{TryRecall, Recallable};
/// use core::fmt;
///
/// #[derive(Debug)]
/// struct Config {
///     concurrency: u32,
/// }
///
/// #[derive(Clone, PartialEq)]
/// struct ConfigMemento {
///     concurrency: u32,
/// }
///
/// #[derive(Debug)]
/// struct RecallError(String);
///
/// impl fmt::Display for RecallError {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "{}", self.0)
///     }
/// }
///
/// impl core::error::Error for RecallError {}
///
/// impl Recallable for Config {
///     type Memento = ConfigMemento;
/// }
///
/// impl From<Config> for ConfigMemento {
///     fn from(c: Config) -> Self {
///         Self { concurrency: c.concurrency }
///     }
/// }
///
/// impl TryRecall for Config {
///     type Error = RecallError;
///
///     fn try_recall(&mut self, memento: Self::Memento) -> Result<(), Self::Error> {
///         if memento.concurrency == 0 {
///             return Err(RecallError("Concurrency must be > 0".into()));
///         }
///         self.concurrency = memento.concurrency;
///         Ok(())
///     }
/// }
///
/// fn main() {
///     let mut config = Config { concurrency: 1 };
///     let valid_memento = ConfigMemento { concurrency: 4 };
///     config.try_recall(valid_memento).unwrap();
///     assert_eq!(config.concurrency, 4);
///
///     let invalid_memento = ConfigMemento { concurrency: 0 };
///     assert!(config.try_recall(invalid_memento).is_err());
/// }
/// ```
pub trait TryRecall: Recallable {
    /// The error type returned when applying a memento fails.
    type Error: core::error::Error + Send + Sync + 'static;

    /// Applies the provided recall to `self`.
    ///
    /// # Errors
    ///
    /// Returns an error if the memento is invalid or cannot be applied.
    #[must_use = "this returns a Result that may contain an error, which should be handled"]
    fn try_recall(&mut self, memento: Self::Memento) -> Result<(), Self::Error>;
}

/// Blanket implementation for all [`Recall`] types, where recalling is
/// infallible.
impl<T: Recall> TryRecall for T {
    type Error = core::convert::Infallible;

    #[inline(always)]
    fn try_recall(&mut self, memento: Self::Memento) -> Result<(), Self::Error> {
        self.recall(memento);
        Ok(())
    }
}
