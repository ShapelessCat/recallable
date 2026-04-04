use proc_macro::TokenStream;

use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use syn::{
    Attribute, Field, Fields, Item, ItemEnum, ItemStruct, Meta, Token, parse::Parse, parse_quote,
    punctuated::Punctuated,
};

use crate::context::{self, SERDE_ENABLED, crate_path};

const DERIVE: &str = "derive";
const SERIALIZE: &str = "Serialize";
const SERDE: &str = "serde";
const SERDE_DERIVE: &str = "serde_derive";

#[must_use]
pub(super) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand_tokens(attr.into(), item.into()) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_tokens(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    validate_model_attr(&attr)?;
    let mut model_item = parse_model_item_tokens(item)?;
    let derive_input = model_item.parse();
    context::analyze_model_input(&derive_input)?;
    if SERDE_ENABLED {
        check_no_serialize_derive(model_item.attrs())?;
    }

    model_item.add_derives();
    if SERDE_ENABLED {
        model_item.add_serde_skip_attrs()?;
    }

    Ok(model_item.item_tokenstream())
}

fn validate_model_attr(attr: &TokenStream2) -> syn::Result<()> {
    if attr.is_empty() {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            attr,
            "`#[recallable_model]` does not accept arguments",
        ))
    }
}

#[must_use]
fn build_model_derive_attr(crate_path: &TokenStream2) -> syn::Attribute {
    if SERDE_ENABLED {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall, ::serde::Serialize)]
        }
    } else {
        parse_quote! {
            #[derive(#crate_path::Recallable, #crate_path::Recall)]
        }
    }
}

fn parse_model_item_tokens(item: TokenStream2) -> syn::Result<ModelItem> {
    let item: Item = syn::parse2(item)?;
    match item {
        Item::Struct(item) => Ok(ModelItem::Struct(item)),
        Item::Enum(item) => Ok(ModelItem::Enum(item)),
        other => Err(syn::Error::new_spanned(
            other,
            "`#[recallable_model]` can only be applied to structs or enums",
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecallableAttrKind {
    Recall,
    Skip,
}

#[derive(Debug, Clone)]
struct ConditionalSerdeState {
    key: String,
    predicate: Option<TokenStream2>,
    saw_recall: bool,
    saw_skip: bool,
    saw_serde_skip: bool,
}

impl ConditionalSerdeState {
    fn new(predicate: Option<TokenStream2>) -> Self {
        let key = predicate
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "<always>".to_owned());
        Self {
            key,
            predicate,
            saw_recall: false,
            saw_skip: false,
            saw_serde_skip: false,
        }
    }
}

struct CfgAttrArgs {
    predicate: Meta,
    attrs: Punctuated<Meta, Token![,]>,
}

impl Parse for CfgAttrArgs {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let predicate = input.parse()?;
        let _: Token![,] = input.parse()?;
        let attrs = Punctuated::parse_terminated(input)?;
        Ok(Self { predicate, attrs })
    }
}

fn add_serde_skip_attrs_to_fields(fields: &mut Fields) -> syn::Result<()> {
    for field in fields.iter_mut() {
        let serde_skip_attrs = serde_skip_attrs_to_add(field)?;
        field.attrs.extend(serde_skip_attrs);
    }

    Ok(())
}

fn serde_skip_attrs_to_add(field: &Field) -> syn::Result<Vec<Attribute>> {
    let mut states = vec![ConditionalSerdeState::new(None)];

    for attr in &field.attrs {
        if attr.path().is_ident("cfg_attr") {
            merge_cfg_attr_state(attr, &mut states)?;
            continue;
        }

        let state = state_for_predicate(&mut states, None);
        if attr.path().is_ident("recallable") {
            record_recallable_attr_state(&attr.meta, state)?;
        } else if attr.path().is_ident("serde") && meta_has_serde_skip(&attr.meta)? {
            state.saw_serde_skip = true;
        }
    }

    let mut serde_skip_attrs = Vec::new();
    for state in states {
        if state.saw_recall && state.saw_skip {
            return Err(syn::Error::new_spanned(
                field,
                "conflicting `recallable` attributes: choose exactly one of `#[recallable]` or `#[recallable(skip)]`",
            ));
        }

        if state.saw_recall && state.saw_serde_skip {
            let target = match &state.predicate {
                Some(predicate) => predicate.clone(),
                None => field.to_token_stream(),
            };
            return Err(syn::Error::new_spanned(
                target,
                "`#[recallable]` cannot coexist with `#[serde(skip)]` on the same field",
            ));
        }

        if state.saw_skip && !state.saw_serde_skip {
            serde_skip_attrs.push(match &state.predicate {
                Some(predicate) => parse_quote!(#[cfg_attr(#predicate, serde(skip))]),
                None => parse_quote!(#[serde(skip)]),
            });
        }
    }

    Ok(serde_skip_attrs)
}

fn state_for_predicate(
    states: &mut Vec<ConditionalSerdeState>,
    predicate: Option<TokenStream2>,
) -> &mut ConditionalSerdeState {
    let key = predicate
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "<always>".to_owned());
    if let Some(index) = states.iter().position(|state| state.key == key) {
        &mut states[index]
    } else {
        states.push(ConditionalSerdeState::new(predicate));
        states.last_mut().expect("newly pushed state is present")
    }
}

fn merge_cfg_attr_state(
    attr: &Attribute,
    states: &mut Vec<ConditionalSerdeState>,
) -> syn::Result<()> {
    let args: CfgAttrArgs = attr.parse_args()?;
    let predicate = args.predicate.to_token_stream();
    let state = state_for_predicate(states, Some(predicate));

    for meta in args.attrs {
        if meta.path().is_ident("recallable") {
            record_recallable_attr_state(&meta, state)?;
        } else if meta.path().is_ident("serde") && meta_has_serde_skip(&meta)? {
            state.saw_serde_skip = true;
        }
    }

    Ok(())
}

fn record_recallable_attr_state(meta: &Meta, state: &mut ConditionalSerdeState) -> syn::Result<()> {
    match classify_recallable_attr(meta)? {
        Some(RecallableAttrKind::Recall) => state.saw_recall = true,
        Some(RecallableAttrKind::Skip) => state.saw_skip = true,
        None => {}
    }
    Ok(())
}

fn classify_recallable_attr(meta: &Meta) -> syn::Result<Option<RecallableAttrKind>> {
    match meta {
        Meta::Path(_) => Ok(Some(RecallableAttrKind::Recall)),
        Meta::List(list) => {
            let mut saw_skip = false;
            list.parse_nested_meta(|nested| {
                if nested.path.is_ident("skip") {
                    saw_skip = true;
                    Ok(())
                } else {
                    Err(nested.error("unrecognized `recallable` parameter"))
                }
            })?;
            Ok(saw_skip.then_some(RecallableAttrKind::Skip))
        }
        Meta::NameValue(_) => Err(syn::Error::new_spanned(
            meta,
            "unrecognized `recallable` parameter",
        )),
    }
}

fn meta_has_serde_skip(meta: &Meta) -> syn::Result<bool> {
    let Meta::List(list) = meta else {
        return Ok(false);
    };

    let mut saw_skip = false;
    list.parse_nested_meta(|nested| {
        if nested.path.is_ident("skip") {
            saw_skip = true;
        }
        Ok(())
    })?;

    Ok(saw_skip)
}

/// Returns an error if any existing `#[derive(...)]` attribute on the struct
/// already includes a serde-backed `Serialize` derive.
///
/// Called only when `SERDE_ENABLED` is true, before `#[recallable_model]`
/// injects its own `::serde::Serialize` derive.
fn check_no_serialize_derive(attrs: &[syn::Attribute]) -> syn::Result<()> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident(DERIVE))
        .try_for_each(|attr| {
            attr.parse_nested_meta(|meta| {
                if is_serde_serialize_path(&meta.path) {
                    Err(meta.error(
                        "`#[recallable_model]` already derives `serde::Serialize` when the \
                         `serde` feature is enabled — remove the manual `Serialize` derive",
                    ))
                } else {
                    Ok(())
                }
            })
        })
}

fn is_serde_serialize_path(path: &syn::Path) -> bool {
    // Attribute macros cannot resolve imported names, so keep treating a bare
    // `Serialize` derive as serde-shaped for the common `use serde::Serialize;` case.
    path.is_ident("Serialize") || {
        let mut segments = path.segments.iter();
        matches!(
            (segments.next(), segments.next(), segments.next()),
            (Some(first), Some(second), None)
                if (first.ident == SERDE || first.ident == SERDE_DERIVE)
                    && second.ident == SERIALIZE
        )
    }
}

enum ModelItem {
    Struct(ItemStruct),
    Enum(ItemEnum),
}

impl ModelItem {
    fn attrs(&self) -> &[syn::Attribute] {
        match self {
            Self::Struct(item) => &item.attrs,
            Self::Enum(item) => &item.attrs,
        }
    }

    fn attrs_mut(&mut self) -> &mut Vec<syn::Attribute> {
        match self {
            Self::Struct(item) => &mut item.attrs,
            Self::Enum(item) => &mut item.attrs,
        }
    }

    fn with_fields_mut(
        &mut self,
        mut apply: impl FnMut(&mut Fields) -> syn::Result<()>,
    ) -> syn::Result<()> {
        match self {
            Self::Struct(item) => apply(&mut item.fields),
            Self::Enum(item) => {
                for variant in &mut item.variants {
                    apply(&mut variant.fields)?;
                }
                Ok(())
            }
        }
    }

    fn add_derives(&mut self) {
        let crate_path = crate_path();
        let derives = build_model_derive_attr(&crate_path);
        self.attrs_mut().push(derives);
    }

    fn add_serde_skip_attrs(&mut self) -> syn::Result<()> {
        self.with_fields_mut(add_serde_skip_attrs_to_fields)
    }

    fn item_tokenstream(&self) -> TokenStream2 {
        match self {
            ModelItem::Struct(item) => item.to_token_stream(),
            ModelItem::Enum(item) => item.to_token_stream(),
        }
    }

    fn parse(&self) -> syn::DeriveInput {
        match self {
            ModelItem::Struct(item) => item.clone().into(),
            ModelItem::Enum(item) => item.clone().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse_quote;

    use super::{
        expand_tokens, is_serde_serialize_path, parse_model_item_tokens, validate_model_attr,
    };

    #[test]
    fn serde_serialize_path_detection_is_precise() {
        assert!(is_serde_serialize_path(&parse_quote!(Serialize)));
        assert!(is_serde_serialize_path(&parse_quote!(serde::Serialize)));
        assert!(is_serde_serialize_path(&parse_quote!(::serde::Serialize)));
        assert!(is_serde_serialize_path(&parse_quote!(
            serde_derive::Serialize
        )));
        assert!(is_serde_serialize_path(&parse_quote!(
            ::serde_derive::Serialize
        )));

        assert!(!is_serde_serialize_path(&parse_quote!(other::Serialize)));
        assert!(!is_serde_serialize_path(&parse_quote!(
            serde::ser::Serialize
        )));
        assert!(!is_serde_serialize_path(&parse_quote!(
            other::serde::Serialize
        )));
        assert!(!is_serde_serialize_path(&parse_quote!(SerializeOwned)));
    }

    #[test]
    fn recallable_model_rejects_arguments() {
        let error = validate_model_attr(&quote!(unexpected)).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("`#[recallable_model]` does not accept arguments")
        );
    }

    #[test]
    fn parse_model_item_rejects_non_struct_or_enum_items() {
        let error = match parse_model_item_tokens(quote!(
            fn example() {}
        )) {
            Ok(_) => panic!("expected parse_model_item to reject functions"),
            Err(error) => error,
        };

        assert_eq!(
            error.to_string(),
            "`#[recallable_model]` can only be applied to structs or enums"
        );
    }

    #[test]
    fn expand_tokens_reject_model_arguments() {
        let error = expand_tokens(
            quote!(unexpected),
            quote!(
                struct Example;
            ),
        )
        .unwrap_err();

        assert!(error.to_string().contains("does not accept arguments"));
    }

    #[test]
    fn expand_tokens_reject_non_model_items() {
        let error = expand_tokens(
            quote!(),
            quote!(
                fn example() {}
            ),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("can only be applied to structs or enums")
        );
    }

    #[test]
    fn expand_tokens_reject_model_analysis_failures() {
        let error = expand_tokens(
            quote!(),
            quote! {
                enum Example {
                    Value(#[recallable] Inner),
                }
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("assignment-only variants"));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn expand_tokens_reject_manual_serialize_derives() {
        let error = expand_tokens(
            quote!(),
            quote! {
                #[derive(serde::Serialize)]
                struct Example {
                    value: u32,
                }
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("already derives"));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn expand_tokens_injects_matching_cfg_attr_serde_skip() {
        let expanded: syn::ItemStruct = syn::parse2(
            expand_tokens(
                quote!(),
                quote! {
                    struct Example {
                        #[cfg_attr(feature = "std", recallable(skip))]
                        value: u32,
                    }
                },
            )
            .unwrap(),
        )
        .unwrap();

        let field = match &expanded.fields {
            syn::Fields::Named(fields) => fields.named.first().unwrap(),
            _ => unreachable!("expected named fields"),
        };
        let attrs = field
            .attrs
            .iter()
            .map(quote::ToTokens::to_token_stream)
            .map(|tokens| tokens.to_string())
            .collect::<Vec<_>>();

        assert!(
            attrs.contains(&quote!(#[cfg_attr(feature = "std", recallable(skip))]).to_string())
        );
        assert!(attrs.contains(&quote!(#[cfg_attr(feature = "std", serde(skip))]).to_string()));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn expand_tokens_does_not_duplicate_cfg_attr_serde_skip_when_already_present() {
        let expanded: syn::ItemStruct = syn::parse2(
            expand_tokens(
                quote!(),
                quote! {
                    struct Example {
                        #[cfg_attr(feature = "std", recallable(skip), serde(skip))]
                        value: u32,
                    }
                },
            )
            .unwrap(),
        )
        .unwrap();

        let field = match &expanded.fields {
            syn::Fields::Named(fields) => fields.named.first().unwrap(),
            _ => unreachable!("expected named fields"),
        };

        assert_eq!(field.attrs.len(), 1);
        assert_eq!(
            quote::ToTokens::to_token_stream(&field.attrs[0]).to_string(),
            quote!(#[cfg_attr(feature = "std", recallable(skip), serde(skip))]).to_string()
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn expand_tokens_rejects_cfg_attr_recallable_and_serde_skip_conflicts() {
        let error = expand_tokens(
            quote!(),
            quote! {
                struct Example {
                    #[cfg_attr(feature = "std", recallable, serde(skip))]
                    value: u32,
                }
            },
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("`#[recallable]` cannot coexist with `#[serde(skip)]`")
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn expand_tokens_rejects_cfg_attr_recallable_unknown_parameters() {
        let error = expand_tokens(
            quote!(),
            quote! {
                struct Example {
                    #[cfg_attr(feature = "std", recallable(skip, garbage))]
                    value: u32,
                }
            },
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "unrecognized `recallable` parameter");
    }
}
