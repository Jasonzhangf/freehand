use super::super::cfg::evaluate_cfg;
use super::*;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Meta, Token};

pub(super) fn has_test_attribute(attrs: &[syn::Attribute], active_cfg: &BTreeSet<String>) -> bool {
    if attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
    }) {
        return true;
    }
    let mut production_cfg = active_cfg.clone();
    production_cfg.remove("test");
    let cfg_predicates = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .filter_map(|attr| attr.parse_args::<Meta>().ok())
        .collect::<Vec<_>>();
    !cfg_predicates.is_empty()
        && cfg_predicates.iter().any(cfg_contains_test)
        && !cfg_predicates
            .iter()
            .all(|meta| evaluate_cfg(meta, &production_cfg).unwrap_or(false))
}

fn cfg_contains_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::NameValue(_) => false,
        Meta::List(list) => Punctuated::<Meta, Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .is_ok_and(|nested| nested.iter().any(cfg_contains_test)),
    }
}
