use super::attributes::has_test_attribute;
use super::methods::receiver::type_identity;
use super::*;

pub(super) fn collect_module_test_relations(
    items: &[Item],
    module: &[String],
    active_cfg: &BTreeSet<String>,
    relations: &mut Vec<(String, String, bool)>,
) -> Result<(), String> {
    let parent = module.join("::");
    for item in items {
        let Item::Mod(item_mod) = item else {
            continue;
        };
        if !active_in_cfg_projection(
            &item_mod.attrs,
            &format!("module declaration in `{parent}`"),
            active_cfg,
        )? {
            continue;
        }
        let mut child = module.to_vec();
        child.push(item_mod.ident.to_string());
        relations.push((
            child.join("::"),
            parent.clone(),
            has_test_attribute(&item_mod.attrs, active_cfg),
        ));
        if let Some((_, nested)) = &item_mod.content {
            collect_module_test_relations(nested, &child, active_cfg, relations)?;
        }
    }
    Ok(())
}

pub(super) fn resolve_test_modules(relations: &[(String, String, bool)]) -> BTreeSet<String> {
    let mut test_modules = relations
        .iter()
        .filter(|(_, _, explicit_test)| *explicit_test)
        .map(|(module, _, _)| module.clone())
        .collect::<BTreeSet<_>>();
    loop {
        let before = test_modules.len();
        let inherited = relations
            .iter()
            .filter(|(_, parent, _)| test_modules.contains(parent))
            .map(|(module, _, _)| module.clone())
            .collect::<Vec<_>>();
        test_modules.extend(inherited);
        if test_modules.len() == before {
            return test_modules;
        }
    }
}

pub(super) fn resolve_reachable_modules(relations: &[(String, String, bool)]) -> BTreeSet<String> {
    let mut reachable = BTreeSet::from(["crate".to_owned()]);
    loop {
        let before = reachable.len();
        for (module, parent, _) in relations {
            if reachable.contains(parent) {
                reachable.insert(module.clone());
            }
        }
        if reachable.len() == before {
            return reachable;
        }
    }
}

pub(super) fn index_module_items(
    items: &[Item],
    module: &[String],
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let module_id = module.join("::");
    state.modules.entry(module_id.clone()).or_default();
    for item in items {
        let Some(attrs) = item_attrs(item) else {
            continue;
        };
        if !active_in_cfg_projection(attrs, &format!("item in `{module_id}`"), state.active_cfg)? {
            continue;
        }
        match item {
            Item::Use(item_use) => {
                let mut imports = Vec::new();
                flatten_use_tree(&item_use.tree, Vec::new(), &mut imports);
                state
                    .modules
                    .get_mut(&module_id)
                    .expect("module initialized")
                    .imports
                    .extend(imports);
            }
            Item::Mod(item_mod) => {
                let mut child = module.to_vec();
                child.push(item_mod.ident.to_string());
                if let Some((_, items)) = &item_mod.content {
                    index_module_items(items, &child, state)?;
                } else {
                    state.declared_external_modules.insert(child.join("::"));
                }
            }
            Item::Struct(value) => {
                state
                    .local_types
                    .insert(format!("{module_id}::{}", value.ident));
            }
            Item::Enum(value) => {
                state
                    .local_types
                    .insert(format!("{module_id}::{}", value.ident));
            }
            Item::Union(value) => {
                state
                    .local_types
                    .insert(format!("{module_id}::{}", value.ident));
            }
            Item::Type(value) => {
                state
                    .local_types
                    .insert(format!("{module_id}::{}", value.ident));
            }
            Item::Trait(value) => {
                let trait_id = format!("{module_id}::{}", value.ident);
                state.local_types.insert(trait_id.clone());
                state.local_traits.insert(trait_id);
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn index_function_return_types(
    items: &[Item],
    module: &[String],
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let module_id = module.join("::");
    for item in items {
        let Some(attrs) = item_attrs(item) else {
            continue;
        };
        if !active_in_cfg_projection(attrs, &format!("item in `{module_id}`"), state.active_cfg)? {
            continue;
        }
        match item {
            Item::Fn(value) => {
                state
                    .modules
                    .get_mut(&module_id)
                    .expect("module indexed")
                    .definitions
                    .insert(
                        value.sig.ident.to_string(),
                        format!("{module_id}::{}", value.sig.ident),
                    );
                let syn::ReturnType::Type(_, ty) = &value.sig.output else {
                    continue;
                };
                if let Some(owner) = type_identity(ty, module, state.modules, state.local_types) {
                    state
                        .function_return_types
                        .insert(format!("{module_id}::{}", value.sig.ident), owner);
                }
                let syn::Type::Path(path) = ty.as_ref() else {
                    continue;
                };
                let Some(last) = path.path.segments.last() else {
                    continue;
                };
                if !matches!(last.ident.to_string().as_str(), "Result" | "Option") {
                    continue;
                }
                let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else {
                    continue;
                };
                let Some(syn::GenericArgument::Type(success_type)) = arguments.args.first() else {
                    continue;
                };
                if let Some(owner) =
                    type_identity(success_type, module, state.modules, state.local_types)
                {
                    state
                        .function_try_return_types
                        .insert(format!("{module_id}::{}", value.sig.ident), owner);
                }
            }
            Item::Mod(value) => {
                let Some((_, items)) = &value.content else {
                    continue;
                };
                let mut child = module.to_vec();
                child.push(value.ident.to_string());
                index_function_return_types(items, &child, state)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn index_struct_fields(
    items: &[Item],
    module: &[String],
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let module_id = module.join("::");
    for item in items {
        let Some(attrs) = item_attrs(item) else {
            continue;
        };
        if !active_in_cfg_projection(attrs, &format!("item in `{module_id}`"), state.active_cfg)? {
            continue;
        }
        match item {
            Item::Struct(value) => {
                let receiver = format!("{module_id}::{}", value.ident);
                for field in &value.fields {
                    let Some(name) = &field.ident else {
                        continue;
                    };
                    let owner: Option<Vec<String>> =
                        type_identity(&field.ty, module, state.modules, state.local_types)
                            .filter(|owner| state.local_types.contains(&owner.join("::")));
                    state
                        .struct_fields
                        .insert((receiver.clone(), name.to_string()), owner);
                }
            }
            Item::Mod(value) => {
                let Some((_, items)) = &value.content else {
                    continue;
                };
                let mut child = module.to_vec();
                child.push(value.ident.to_string());
                index_struct_fields(items, &child, state)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn item_attrs(item: &Item) -> Option<&[syn::Attribute]> {
    match item {
        Item::Fn(value) => Some(&value.attrs),
        Item::Mod(value) => Some(&value.attrs),
        Item::Use(value) => Some(&value.attrs),
        Item::Impl(value) => Some(&value.attrs),
        Item::Trait(value) => Some(&value.attrs),
        Item::Const(value) => Some(&value.attrs),
        Item::Static(value) => Some(&value.attrs),
        Item::Struct(value) => Some(&value.attrs),
        Item::Enum(value) => Some(&value.attrs),
        Item::Union(value) => Some(&value.attrs),
        Item::Type(value) => Some(&value.attrs),
        _ => None,
    }
}
