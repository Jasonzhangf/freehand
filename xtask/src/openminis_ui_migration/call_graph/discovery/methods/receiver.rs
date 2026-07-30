use super::super::scope::resolve_call_path;
use super::*;
use syn::{Expr, Type};

pub(super) fn signature_local_types(
    sig: &syn::Signature,
    module: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    repository_types: &BTreeSet<String>,
) -> BTreeMap<String, Vec<String>> {
    sig.inputs
        .iter()
        .filter_map(|arg| {
            let syn::FnArg::Typed(arg) = arg else {
                return None;
            };
            let syn::Pat::Ident(name) = arg.pat.as_ref() else {
                return None;
            };
            type_identity(arg.ty.as_ref(), module, modules, repository_types)
                .map(|owner| (name.ident.to_string(), owner))
        })
        .collect()
}

pub(super) fn signature_external_locals(sig: &syn::Signature) -> BTreeSet<String> {
    sig.inputs
        .iter()
        .filter_map(|arg| {
            let syn::FnArg::Typed(arg) = arg else {
                return None;
            };
            let syn::Pat::Ident(name) = arg.pat.as_ref() else {
                return None;
            };
            type_is_builtin_external_receiver(arg.ty.as_ref()).then(|| name.ident.to_string())
        })
        .collect()
}

pub(super) fn signature_iterable_item_types(
    sig: &syn::Signature,
    module: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    repository_types: &BTreeSet<String>,
) -> BTreeMap<String, Vec<String>> {
    sig.inputs
        .iter()
        .filter_map(|arg| {
            let syn::FnArg::Typed(arg) = arg else {
                return None;
            };
            let syn::Pat::Ident(name) = arg.pat.as_ref() else {
                return None;
            };
            iterable_item_type(arg.ty.as_ref(), module, modules, repository_types)
                .map(|owner| (name.ident.to_string(), owner))
        })
        .collect()
}

pub(super) fn iterable_item_type(
    ty: &Type,
    module: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    repository_types: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let ty = match ty {
        Type::Reference(reference) => reference.elem.as_ref(),
        _ => ty,
    };
    let item = match ty {
        Type::Array(array) => array.elem.as_ref(),
        Type::Slice(slice) => slice.elem.as_ref(),
        Type::Path(path) => {
            let segment = path.path.segments.last()?;
            if !matches!(
                segment.ident.to_string().as_str(),
                "Vec" | "VecDeque" | "LinkedList" | "HashSet" | "BTreeSet"
            ) {
                return None;
            }
            let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return None;
            };
            arguments.args.iter().find_map(|argument| match argument {
                syn::GenericArgument::Type(item) => Some(item),
                _ => None,
            })?
        }
        _ => return None,
    };
    type_identity(item, module, modules, repository_types)
        .filter(|owner| repository_types.contains(&owner.join("::")))
}

pub(super) fn type_is_builtin_external_receiver(ty: &Type) -> bool {
    let ty = match ty {
        Type::Reference(reference) => reference.elem.as_ref(),
        _ => ty,
    };
    matches!(
        ty,
        Type::Array(_)
            | Type::BareFn(_)
            | Type::Never(_)
            | Type::Ptr(_)
            | Type::Slice(_)
            | Type::Tuple(_)
    )
}

pub(super) fn local_type(
    local: &syn::Local,
    module: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    repository_types: &BTreeSet<String>,
    function_return_types: &BTreeMap<String, Vec<String>>,
    function_try_return_types: &BTreeMap<String, Vec<String>>,
) -> Option<(String, Vec<String>)> {
    let name = match &local.pat {
        syn::Pat::Ident(name) => &name.ident,
        syn::Pat::Type(typed) => {
            let syn::Pat::Ident(name) = typed.pat.as_ref() else {
                return None;
            };
            return type_identity(typed.ty.as_ref(), module, modules, repository_types)
                .map(|owner| (name.ident.to_string(), owner));
        }
        _ => return None,
    };
    expression_type(
        local.init.as_ref()?.expr.as_ref(),
        module,
        modules,
        repository_types,
        function_return_types,
        function_try_return_types,
    )
    .map(|owner| (name.to_string(), owner))
}

pub(super) fn expression_type(
    expression: &Expr,
    module: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    repository_types: &BTreeSet<String>,
    function_return_types: &BTreeMap<String, Vec<String>>,
    function_try_return_types: &BTreeMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    match expression {
        Expr::Call(call) => {
            call_return_type(call, module, modules, function_return_types).or_else(|| {
                let Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                let mut segments = path
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>();
                segments.pop();
                (!segments.is_empty())
                    .then(|| resolve_type_path(module, &segments, modules, repository_types))
            })
        }
        Expr::Try(value) => {
            let Expr::Call(call) = value.expr.as_ref() else {
                return None;
            };
            call_return_type(call, module, modules, function_try_return_types)
        }
        Expr::MethodCall(value)
            if matches!(value.method.to_string().as_str(), "unwrap" | "expect") =>
        {
            let Expr::Call(call) = value.receiver.as_ref() else {
                return None;
            };
            call_return_type(call, module, modules, function_try_return_types)
        }
        Expr::Struct(value) => {
            let segments = value
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            Some(resolve_type_path(
                module,
                &segments,
                modules,
                repository_types,
            ))
        }
        Expr::Path(value) if value.qself.is_none() => {
            let segments = value
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            let owner = resolve_type_path(module, &segments, modules, repository_types);
            repository_types
                .contains(&owner.join("::"))
                .then_some(owner)
        }
        Expr::If(value) => {
            let then_type = block_tail_type(
                &value.then_branch,
                module,
                modules,
                repository_types,
                function_return_types,
                function_try_return_types,
            )?;
            let else_type = value.else_branch.as_ref().and_then(|(_, branch)| {
                expression_type(
                    branch,
                    module,
                    modules,
                    repository_types,
                    function_return_types,
                    function_try_return_types,
                )
            })?;
            (then_type == else_type).then_some(then_type)
        }
        Expr::Match(value) => {
            let mut arm_types = value.arms.iter().map(|arm| {
                expression_type(
                    &arm.body,
                    module,
                    modules,
                    repository_types,
                    function_return_types,
                    function_try_return_types,
                )
            });
            let first = arm_types.next()??;
            arm_types
                .all(|owner| owner.as_ref() == Some(&first))
                .then_some(first)
        }
        Expr::Block(value) => block_tail_type(
            &value.block,
            module,
            modules,
            repository_types,
            function_return_types,
            function_try_return_types,
        ),
        Expr::Group(value) => expression_type(
            &value.expr,
            module,
            modules,
            repository_types,
            function_return_types,
            function_try_return_types,
        ),
        Expr::Paren(value) => expression_type(
            &value.expr,
            module,
            modules,
            repository_types,
            function_return_types,
            function_try_return_types,
        ),
        Expr::Reference(value) => expression_type(
            &value.expr,
            module,
            modules,
            repository_types,
            function_return_types,
            function_try_return_types,
        ),
        _ => None,
    }
}

fn block_tail_type(
    block: &syn::Block,
    module: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    repository_types: &BTreeSet<String>,
    function_return_types: &BTreeMap<String, Vec<String>>,
    function_try_return_types: &BTreeMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    let Stmt::Expr(expression, None) = block.stmts.last()? else {
        return None;
    };
    expression_type(
        expression,
        module,
        modules,
        repository_types,
        function_return_types,
        function_try_return_types,
    )
}

pub(super) fn call_return_type(
    call: &syn::ExprCall,
    module: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    function_return_types: &BTreeMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    let Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let resolved = resolve_call_path(
        module,
        &segments,
        modules,
        &BTreeMap::from_iter(
            function_return_types
                .keys()
                .map(|symbol| (symbol.clone(), String::new())),
        ),
    )
    .ok()?;
    if resolved.len() != 1 {
        return None;
    }
    function_return_types
        .get(resolved.first().expect("one return-type function"))
        .cloned()
}

pub(super) fn expression_starts_with_local_receiver(
    expression: &Expr,
    module: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    repository_types: &BTreeSet<String>,
    local_types: &BTreeMap<String, Vec<String>>,
    function_return_types: &BTreeMap<String, Vec<String>>,
    callable_receiver_type: Option<&Vec<String>>,
) -> bool {
    match expression {
        Expr::Path(path) if path.path.is_ident("self") => {
            callable_receiver_type.is_some_and(|owner| repository_types.contains(&owner.join("::")))
        }
        Expr::Path(path) => path
            .path
            .get_ident()
            .and_then(|ident| local_types.get(&ident.to_string()))
            .is_some_and(|owner| repository_types.contains(&owner.join("::"))),
        Expr::Call(call) => call_return_type(call, module, modules, function_return_types)
            .is_some_and(|owner| repository_types.contains(&owner.join("::"))),
        Expr::MethodCall(call) => expression_starts_with_local_receiver(
            &call.receiver,
            module,
            modules,
            repository_types,
            local_types,
            function_return_types,
            callable_receiver_type,
        ),
        Expr::Field(field) => expression_starts_with_local_receiver(
            &field.base,
            module,
            modules,
            repository_types,
            local_types,
            function_return_types,
            callable_receiver_type,
        ),
        Expr::Group(group) => expression_starts_with_local_receiver(
            &group.expr,
            module,
            modules,
            repository_types,
            local_types,
            function_return_types,
            callable_receiver_type,
        ),
        Expr::Paren(paren) => expression_starts_with_local_receiver(
            &paren.expr,
            module,
            modules,
            repository_types,
            local_types,
            function_return_types,
            callable_receiver_type,
        ),
        Expr::Reference(reference) => expression_starts_with_local_receiver(
            &reference.expr,
            module,
            modules,
            repository_types,
            local_types,
            function_return_types,
            callable_receiver_type,
        ),
        _ => false,
    }
}

pub(in crate::openminis_ui_migration::call_graph::discovery) fn type_identity(
    ty: &Type,
    module: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    repository_types: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let ty = match ty {
        Type::Reference(reference) => reference.elem.as_ref(),
        _ => ty,
    };
    let Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?;
    if matches!(
        last.ident.to_string().as_str(),
        "Box"
            | "Arc"
            | "Rc"
            | "Pin"
            | "Cow"
            | "MutexGuard"
            | "RwLockReadGuard"
            | "RwLockWriteGuard"
            | "Ref"
            | "RefMut"
    ) && let syn::PathArguments::AngleBracketed(arguments) = &last.arguments
        && let Some(inner) = arguments.args.iter().find_map(|argument| match argument {
            syn::GenericArgument::Type(inner) => {
                type_identity(inner, module, modules, repository_types)
            }
            _ => None,
        })
    {
        return Some(inner);
    }
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    Some(resolve_type_path(
        module,
        &segments,
        modules,
        repository_types,
    ))
}

pub(super) fn resolve_type_path(
    module: &[String],
    raw: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    repository_types: &BTreeSet<String>,
) -> Vec<String> {
    if let Some(first) = raw.first()
        && let Some(scope) = modules.get(&module.join("::"))
    {
        for import in &scope.imports {
            if !import.glob && import.alias.as_ref() == Some(first) {
                let mut target = normalize_use_target(module, &import.target);
                target.extend_from_slice(&raw[1..]);
                if repository_types.contains(&target.join("::")) {
                    return target;
                }
            }
        }
        if raw.len() == 1 {
            let mut glob_matches = scope
                .imports
                .iter()
                .filter(|import| import.glob)
                .filter_map(|import| {
                    let mut target = normalize_use_target(module, &import.target);
                    target.push(first.clone());
                    repository_types
                        .contains(&target.join("::"))
                        .then_some(target)
                })
                .collect::<Vec<_>>();
            if glob_matches.len() == 1 {
                return glob_matches.pop().expect("one glob type match");
            }
        }
    }
    normalize_use_target(module, raw)
}
