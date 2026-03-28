use std::collections::{HashMap, HashSet};

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::visit::Visit;
use syn::{GenericParam, Generics, Ident, PathArguments, Type, WhereClause, WherePredicate};

use crate::context::internal::shared::FieldIr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenericParamRetention {
    Dropped,
    Retained,
    RetainedAsRecallable,
}

#[derive(Debug)]
pub(crate) struct GenericParamPlan<'a> {
    pub(crate) param: &'a GenericParam,
    retention: GenericParamRetention,
}

impl<'a> GenericParamPlan<'a> {
    #[must_use]
    pub(crate) const fn is_retained(&self) -> bool {
        !matches!(self.retention, GenericParamRetention::Dropped)
    }

    pub(crate) const fn decl_param(&self) -> &GenericParam {
        self.param
    }

    #[must_use]
    pub(crate) fn type_arg(&self) -> TokenStream2 {
        match self.param {
            GenericParam::Lifetime(param) => {
                let lifetime = &param.lifetime;
                quote! { #lifetime }
            }
            GenericParam::Type(param) => {
                let ident = &param.ident;
                quote! { #ident }
            }
            GenericParam::Const(param) => {
                let ident = &param.ident;
                quote! { #ident }
            }
        }
    }

    #[must_use]
    pub(crate) const fn recallable_ident(&self) -> Option<&'a Ident> {
        match (self.param, self.retention) {
            (GenericParam::Type(param), GenericParamRetention::RetainedAsRecallable) => {
                Some(&param.ident)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct GenericUsage {
    pub(crate) retained: HashSet<usize>,
    pub(crate) recallable_type_params: HashSet<usize>,
}

#[derive(Debug)]
pub(crate) struct GenericParamLookup<'a> {
    type_params: HashMap<&'a Ident, usize>,
    const_params: HashMap<&'a Ident, usize>,
    lifetime_params: HashMap<&'a Ident, usize>,
}

impl<'a> GenericParamLookup<'a> {
    #[must_use]
    pub(crate) fn new(generics: &'a Generics) -> Self {
        let mut type_params = HashMap::new();
        let mut const_params = HashMap::new();
        let mut lifetime_params = HashMap::new();

        for (index, param) in generics.params.iter().enumerate() {
            match param {
                GenericParam::Lifetime(param) => {
                    lifetime_params.insert(&param.lifetime.ident, index);
                }
                GenericParam::Type(param) => {
                    type_params.insert(&param.ident, index);
                }
                GenericParam::Const(param) => {
                    const_params.insert(&param.ident, index);
                }
            }
        }

        Self {
            type_params,
            const_params,
            lifetime_params,
        }
    }

    #[must_use]
    pub(crate) fn type_param_index(&self, ident: &Ident) -> Option<usize> {
        self.type_params.get(ident).copied()
    }

    fn const_param_index(&self, ident: &Ident) -> Option<usize> {
        self.const_params.get(ident).copied()
    }
}

pub(crate) struct BareTypeParam(pub(crate) usize);

#[must_use]
pub(crate) fn collect_marker_param_indices<'field, 'input>(
    fields: impl IntoIterator<Item = &'field FieldIr<'input>>,
    generic_params: &[GenericParamPlan<'input>],
    generic_lookup: &GenericParamLookup<'input>,
) -> Vec<usize>
where
    'input: 'field,
{
    let referenced_by_fields: HashSet<_> = fields
        .into_iter()
        .filter(|field| !field.strategy.is_skip())
        .flat_map(|field| collect_generic_dependencies_in_type(field.ty, generic_lookup))
        .collect();

    generic_params
        .iter()
        .enumerate()
        .filter_map(|(index, plan)| {
            (plan.is_retained() && !referenced_by_fields.contains(&index)).then_some(index)
        })
        .collect()
}

#[must_use]
pub(crate) fn plan_memento_generics<'a>(
    generics: &'a Generics,
    mut usage: GenericUsage,
    generic_lookup: &GenericParamLookup<'a>,
) -> (Vec<GenericParamPlan<'a>>, Option<WhereClause>) {
    let param_dependencies: Vec<_> = generics
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let mut deps = collect_generic_dependencies_in_param(param, generic_lookup);
            deps.remove(&index);
            deps
        })
        .collect();

    let predicates_with_deps: Vec<_> = generics
        .where_clause
        .as_ref()
        .map(|where_clause| {
            where_clause
                .predicates
                .iter()
                .map(|predicate| {
                    (
                        predicate,
                        collect_generic_dependencies_in_where_predicate(predicate, generic_lookup),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    let mut kept_predicates = vec![false; predicates_with_deps.len()];

    loop {
        let mut changed = false;

        let retained_now: Vec<_> = usage.retained.iter().copied().collect();
        for index in retained_now {
            for dependency in &param_dependencies[index] {
                changed |= usage.retained.insert(*dependency);
            }
        }

        for (idx, (_, dependencies)) in predicates_with_deps.iter().enumerate() {
            if dependencies.is_empty() {
                continue;
            }
            if dependencies
                .iter()
                .any(|dependency| usage.retained.contains(dependency))
            {
                if !kept_predicates[idx] {
                    kept_predicates[idx] = true;
                    changed = true;
                }
                for dependency in dependencies {
                    changed |= usage.retained.insert(*dependency);
                }
            }
        }

        if !changed {
            break;
        }
    }

    let generic_params = generics
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let retention = if usage.retained.contains(&index) {
                if matches!(param, GenericParam::Type(_))
                    && usage.recallable_type_params.contains(&index)
                {
                    GenericParamRetention::RetainedAsRecallable
                } else {
                    GenericParamRetention::Retained
                }
            } else {
                GenericParamRetention::Dropped
            };
            GenericParamPlan { param, retention }
        })
        .collect();

    let memento_where_clause = generics.where_clause.clone().and_then(|mut where_clause| {
        where_clause.predicates = where_clause
            .predicates
            .into_iter()
            .enumerate()
            .filter_map(|(idx, predicate)| kept_predicates[idx].then_some(predicate))
            .collect();

        (!where_clause.predicates.is_empty()).then_some(where_clause)
    });

    (generic_params, memento_where_clause)
}

struct GenericDependencyCollector<'a> {
    lookup: &'a GenericParamLookup<'a>,
    dependencies: HashSet<usize>,
    angle_arg_depth: usize,
}

impl<'a> GenericDependencyCollector<'a> {
    fn new(lookup: &'a GenericParamLookup<'a>) -> Self {
        Self {
            lookup,
            dependencies: HashSet::new(),
            angle_arg_depth: 0,
        }
    }
}

impl<'ast, 'a> Visit<'ast> for GenericDependencyCollector<'a> {
    fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
        if let Some(index) = self.lookup.lifetime_params.get(&lifetime.ident).copied() {
            self.dependencies.insert(index);
        }
        syn::visit::visit_lifetime(self, lifetime);
    }

    fn visit_angle_bracketed_generic_arguments(
        &mut self,
        node: &'ast syn::AngleBracketedGenericArguments,
    ) {
        self.angle_arg_depth += 1;
        syn::visit::visit_angle_bracketed_generic_arguments(self, node);
        self.angle_arg_depth -= 1;
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if node.qself.is_none()
            && let Some(first_segment) = node.path.segments.first()
        {
            if let Some(index) = self.lookup.type_param_index(&first_segment.ident) {
                self.dependencies.insert(index);
            } else if self.angle_arg_depth > 0
                && node.path.segments.len() == 1
                && matches!(first_segment.arguments, PathArguments::None)
                && let Some(index) = self.lookup.const_param_index(&first_segment.ident)
            {
                self.dependencies.insert(index);
            }
        }

        syn::visit::visit_type_path(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.qself.is_none() && node.path.segments.len() == 1 {
            let ident = &node.path.segments[0].ident;
            if let Some(index) = self.lookup.const_param_index(ident) {
                self.dependencies.insert(index);
            }
        }

        syn::visit::visit_expr_path(self, node);
    }
}

#[must_use]
pub(crate) fn collect_generic_dependencies_in_type(
    ty: &Type,
    generic_lookup: &GenericParamLookup<'_>,
) -> HashSet<usize> {
    let mut collector = GenericDependencyCollector::new(generic_lookup);
    collector.visit_type(ty);
    collector.dependencies
}

fn collect_generic_dependencies_in_param(
    param: &GenericParam,
    generic_lookup: &GenericParamLookup<'_>,
) -> HashSet<usize> {
    let mut collector = GenericDependencyCollector::new(generic_lookup);
    collector.visit_generic_param(param);
    collector.dependencies
}

fn collect_generic_dependencies_in_where_predicate(
    predicate: &WherePredicate,
    generic_lookup: &GenericParamLookup<'_>,
) -> HashSet<usize> {
    let mut collector = GenericDependencyCollector::new(generic_lookup);
    collector.visit_where_predicate(predicate);
    collector.dependencies
}

#[must_use]
pub(crate) fn marker_component(param: &GenericParam) -> TokenStream2 {
    match param {
        GenericParam::Lifetime(param) => {
            let lifetime = &param.lifetime;
            quote! { fn() -> &#lifetime () }
        }
        GenericParam::Type(param) => {
            let ident = &param.ident;
            quote! { fn() -> #ident }
        }
        GenericParam::Const(param) => {
            let ident = &param.ident;
            quote! { [(); { let _ = #ident; 0usize }] }
        }
    }
}

#[must_use]
pub(crate) fn is_generic_type_param(ty: &Type, generic_type_params: &HashSet<&Ident>) -> bool {
    match ty {
        Type::Path(tp) if tp.qself.is_none() && tp.path.segments.len() == 1 => {
            let segment = &tp.path.segments[0];
            matches!(segment.arguments, PathArguments::None)
                && generic_type_params.contains(&segment.ident)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use quote::{ToTokens, quote};
    use syn::parse_quote;

    use crate::context::internal::shared::CodegenItemIr;
    use crate::context::internal::structs::StructIr;

    #[test]
    fn memento_generics_preserve_retained_bounds_defaults_and_where_clauses() {
        let input = parse_quote! {
            struct Example<T: Clone = i32, U, const N: usize = 4>
            where
                T: ::core::convert::From<U>,
                U: Copy,
                [u8; N]: Copy,
            {
                value: T,
                bytes: [u8; N],
                #[recallable(skip)]
                marker: ::core::marker::PhantomData<U>,
            }
        };

        let ir = StructIr::analyze(&input).unwrap();

        assert_eq!(
            ir.memento_decl_generics().to_string(),
            quote!(<T: Clone = i32, U, const N: usize = 4>).to_string()
        );
        assert_eq!(
            ir.memento_where_clause()
                .unwrap()
                .to_token_stream()
                .to_string(),
            quote!(
                where
                    T: ::core::convert::From<U>,
                    U: Copy,
                    [u8; N]: Copy
            )
            .to_string()
        );
        assert_eq!(
            ir.memento_type().to_string(),
            quote!(ExampleMemento<T, U, N>).to_string()
        );
    }

    #[test]
    fn memento_where_clause_filters_predicates_for_dropped_params() {
        let input = parse_quote! {
            struct Example<T, U>
            where
                T: Clone,
                U: Copy,
            {
                value: T,
                #[recallable(skip)]
                marker: ::core::marker::PhantomData<U>,
            }
        };

        let ir = StructIr::analyze(&input).unwrap();

        assert_eq!(
            ir.memento_decl_generics().to_string(),
            quote!(<T>).to_string()
        );
        assert_eq!(
            ir.memento_where_clause()
                .unwrap()
                .to_token_stream()
                .to_string(),
            quote!(where T: Clone).to_string()
        );
        assert_eq!(
            ir.memento_type().to_string(),
            quote!(ExampleMemento<T>).to_string()
        );
    }
}
