use super::*;

pub(super) fn collect_callable_impl(
    source: CallableSource<'_>,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    if !active_in_cfg_projection(
        source.attrs,
        &format!("callable `{}`", source.id),
        state.active_cfg,
    )? {
        return Ok(());
    }
    if let Some(previous) = state
        .definitions
        .insert(source.id.clone(), source.owner.to_owned())
    {
        return Err(format!(
            "duplicate module-qualified Rust function `{}` in `{previous}` and `{}`",
            source.id, source.owner
        ));
    }
    let mut visitor = FunctionCallVisitor::from_source(&source, state);
    visitor.visit_block(source.block);
    if let Some(err) = visitor.visitor_error {
        return Err(err);
    }
    let call_scope = source.id.split("::").map(ToOwned::to_owned).collect();
    state.functions.push(RustFunctionCalls {
        id: source.id,
        module: source.module.to_vec(),
        call_scope,
        paths: visitor.paths,
        typed_method_paths: visitor.typed_method_paths,
        unresolved_method_names: visitor.unresolved_method_names,
        method_owner: source.method_owner,
        receiver_type: source.receiver_type,
        self_method_names: visitor.self_method_names,
        is_test: source.is_test,
    });
    Ok(())
}

pub(in crate::openminis_ui_migration::call_graph::discovery) fn collect_initializer_impl(
    source: InitializerSource<'_>,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    if !active_in_cfg_projection(
        source.attrs,
        &format!("initializer `{}`", source.id),
        state.active_cfg,
    )? {
        return Ok(());
    }
    if let Some(previous) = state
        .definitions
        .insert(source.id.clone(), source.owner.to_owned())
    {
        return Err(format!(
            "duplicate module-qualified Rust initializer `{}` in `{previous}` and `{}`",
            source.id, source.owner
        ));
    }
    let is_test = source.inherited_test || has_test_attribute(source.attrs, state.active_cfg);
    let mut visitor =
        FunctionCallVisitor::from_initializer(source.id.clone(), source.module, is_test, state);
    visitor.visit_expr(source.expression);
    if let Some(err) = visitor.visitor_error {
        return Err(err);
    }
    state.functions.push(RustFunctionCalls {
        id: source.id,
        module: source.module.to_vec(),
        call_scope: source.call_scope.to_vec(),
        paths: visitor.paths,
        typed_method_paths: visitor.typed_method_paths,
        unresolved_method_names: visitor.unresolved_method_names,
        method_owner: source.method_owner,
        receiver_type: source.receiver_type,
        self_method_names: visitor.self_method_names,
        is_test,
    });
    Ok(())
}
