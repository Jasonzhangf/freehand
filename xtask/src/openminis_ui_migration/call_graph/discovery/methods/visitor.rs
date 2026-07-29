use super::super::macros::inspect_macro_expressions;
use super::super::*;
use super::bindings::{
    apply_pattern_binding_state, collect_block_scope_bindings,
    expression_starts_with_known_external_receiver, local_identifier, pattern_binding_types,
    pattern_identifiers, resolve_lexical_import, signature_binding_names,
};
use super::receiver::{
    call_return_type, expression_starts_with_local_receiver, iterable_item_type, local_type,
    resolve_type_path, signature_external_locals, signature_iterable_item_types,
    signature_local_types, type_is_builtin_external_receiver,
};
use super::{CallableSource, InitializerSource};
use crate::openminis_ui_migration::call_graph::cfg::cfg_enabled;
use syn::visit::{self, Visit};
use syn::{Expr, ItemFn, ItemImpl, ItemTrait, Pat, Stmt};

mod assignment;
mod conditions;
mod entry;
pub(super) use conditions::expression_attrs;
pub(in crate::openminis_ui_migration::call_graph::discovery) use entry::collect_initializer_impl;

struct FunctionCallVisitor<'a> {
    paths: Vec<Vec<String>>,
    typed_method_paths: Vec<Vec<String>>,
    unresolved_method_names: Vec<String>,
    self_method_names: Vec<String>,
    visitor_error: Option<String>,
    active_cfg: BTreeSet<String>,
    caller_id: String,
    local_types: BTreeMap<String, Vec<String>>,
    known_external_locals: BTreeSet<String>,
    iterable_item_types: BTreeMap<String, Vec<String>>,
    bound_values: BTreeSet<String>,
    lexical_imports: Vec<RustImport>,
    module: Vec<String>,
    modules: &'a BTreeMap<String, RustModuleScope>,
    definitions: &'a BTreeMap<String, String>,
    repository_types: &'a BTreeSet<String>,
    repository_traits: &'a BTreeSet<String>,
    function_return_types: &'a BTreeMap<String, Vec<String>>,
    function_try_return_types: &'a BTreeMap<String, Vec<String>>,
    callable_receiver_type: Option<Vec<String>>,
    struct_fields: &'a BTreeMap<(String, String), Option<Vec<String>>>,
}

pub(in crate::openminis_ui_migration::call_graph::discovery) fn collect_callable(
    source: CallableSource<'_>,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    entry::collect_callable_impl(source, state)
}

impl<'ast, 'a> Visit<'ast> for FunctionCallVisitor<'a> {
    fn visit_expr(&mut self, node: &'ast Expr) {
        match cfg_enabled(
            conditions::expression_attrs(node),
            &self.active_cfg,
            &format!("expression in module-qualified caller `{}`", self.caller_id),
        ) {
            Ok(true) => visit::visit_expr(self, node),
            Ok(false) => {}
            Err(err) => self.visitor_error = Some(err),
        }
    }

    fn visit_stmt(&mut self, node: &'ast Stmt) {
        self.visit_cfg_statement(node);
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        self.visit_lexical_block(node);
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        let outer_types = self.local_types.clone();
        let outer_external_locals = self.known_external_locals.clone();
        let outer_iterable_item_types = self.iterable_item_types.clone();
        let outer_bound_values = self.bound_values.clone();
        for input in &node.inputs {
            for name in pattern_identifiers(input) {
                self.local_types.remove(&name);
                self.known_external_locals.remove(&name);
                self.iterable_item_types.remove(&name);
                self.bound_values.insert(name);
            }
        }
        for input in &node.inputs {
            let Pat::Type(typed) = input else {
                continue;
            };
            let Pat::Ident(name) = typed.pat.as_ref() else {
                continue;
            };
            let name = name.ident.to_string();
            if let Some(item_owner) = iterable_item_type(
                typed.ty.as_ref(),
                &self.module,
                self.modules,
                self.repository_types,
            ) {
                self.iterable_item_types.insert(name.clone(), item_owner);
            }
            if let Some(owner) = super::receiver::type_identity(
                typed.ty.as_ref(),
                &self.module,
                self.modules,
                self.repository_types,
            ) {
                self.known_external_locals.remove(&name);
                self.local_types.insert(name, owner);
            } else if type_is_builtin_external_receiver(typed.ty.as_ref()) {
                self.local_types.remove(&name);
                self.known_external_locals.insert(name);
            }
        }
        visit::visit_expr_closure(self, node);
        self.local_types = outer_types;
        self.known_external_locals = outer_external_locals;
        self.iterable_item_types = outer_iterable_item_types;
        self.bound_values = outer_bound_values;
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        let outer_types = self.local_types.clone();
        let outer_external_locals = self.known_external_locals.clone();
        let outer_iterable_item_types = self.iterable_item_types.clone();
        let outer_bound_values = self.bound_values.clone();
        self.visit_expr(&node.expr);
        let item_owner = match &*node.expr {
            Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|name| self.iterable_item_types.get(&name.to_string()))
                .cloned(),
            _ => None,
        };
        let definitely_external = expression_starts_with_known_external_receiver(
            &node.expr,
            &self.local_types,
            &self.known_external_locals,
            self.repository_types,
        );
        let typed_bindings = item_owner
            .as_ref()
            .map(|owner| pattern_binding_types(&node.pat, owner, self.struct_fields));
        for name in pattern_identifiers(&node.pat) {
            apply_pattern_binding_state(
                name.clone(),
                typed_bindings
                    .as_ref()
                    .and_then(|bindings| bindings.get(&name)),
                definitely_external,
                &mut self.local_types,
                &mut self.known_external_locals,
                &mut self.iterable_item_types,
                &mut self.bound_values,
            );
        }
        visit::visit_block(self, &node.body);
        self.local_types = outer_types;
        self.known_external_locals = outer_external_locals;
        self.iterable_item_types = outer_iterable_item_types;
        self.bound_values = outer_bound_values;
    }

    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        match cfg_enabled(
            &node.attrs,
            &self.active_cfg,
            &format!("match arm in module-qualified caller `{}`", self.caller_id),
        ) {
            Ok(true) => {}
            Ok(false) => return,
            Err(err) => {
                self.visitor_error = Some(err);
                return;
            }
        }
        let outer_types = self.local_types.clone();
        let outer_external_locals = self.known_external_locals.clone();
        let outer_iterable_item_types = self.iterable_item_types.clone();
        let outer_bound_values = self.bound_values.clone();
        for name in pattern_identifiers(&node.pat) {
            self.local_types.remove(&name);
            self.known_external_locals.remove(&name);
            self.iterable_item_types.remove(&name);
            self.bound_values.insert(name);
        }
        visit::visit_arm(self, node);
        self.local_types = outer_types;
        self.known_external_locals = outer_external_locals;
        self.iterable_item_types = outer_iterable_item_types;
        self.bound_values = outer_bound_values;
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        let outer_types = self.local_types.clone();
        let outer_external_locals = self.known_external_locals.clone();
        let outer_iterable_item_types = self.iterable_item_types.clone();
        let outer_bound_values = self.bound_values.clone();
        self.visit_condition(&node.cond);
        self.visit_block(&node.then_branch);
        self.local_types = outer_types;
        self.known_external_locals = outer_external_locals;
        self.iterable_item_types = outer_iterable_item_types;
        self.bound_values = outer_bound_values;
        if let Some((_, alternate)) = &node.else_branch {
            self.visit_expr(alternate);
        }
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        let outer_types = self.local_types.clone();
        let outer_external_locals = self.known_external_locals.clone();
        let outer_iterable_item_types = self.iterable_item_types.clone();
        let outer_bound_values = self.bound_values.clone();
        self.visit_condition(&node.cond);
        self.visit_block(&node.body);
        self.local_types = outer_types;
        self.known_external_locals = outer_external_locals;
        self.iterable_item_types = outer_iterable_item_types;
        self.bound_values = outer_bound_values;
    }

    fn visit_item_impl(&mut self, _node: &'ast ItemImpl) {}

    fn visit_item_trait(&mut self, _node: &'ast ItemTrait) {}

    fn visit_item_const(&mut self, _node: &'ast syn::ItemConst) {}

    fn visit_item_static(&mut self, _node: &'ast syn::ItemStatic) {}

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if self.visitor_error.is_some() {
            return;
        }
        let context = format!(
            "nested function `{}` in module-qualified caller `{}`",
            node.sig.ident, self.caller_id
        );
        match active_in_cfg_projection(&node.attrs, &context, &self.active_cfg) {
            Ok(false) => {}
            Ok(true) => {
                self.visitor_error = Some(format!(
                    "{context} is unsupported because it has no independent module-qualified caller identity"
                ));
            }
            Err(err) => self.visitor_error = Some(err),
        }
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        let mut callee = node.func.as_ref();
        loop {
            callee = match callee {
                Expr::Paren(value) => &value.expr,
                Expr::Group(value) => &value.expr,
                _ => break,
            };
        }
        if let Expr::Path(path) = callee {
            if path.qself.is_some() {
                self.visitor_error = Some(format!(
                    "qualified UFCS call `{}` in module-qualified caller `{}` is unsupported by the active call graph",
                    path.path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::"),
                    self.caller_id
                ));
                return;
            }
            let mut segments = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            if segments.len() > 1 {
                let owner = resolve_type_path(
                    &self.module,
                    &segments[..segments.len() - 1],
                    self.modules,
                    self.repository_types,
                );
                if self.repository_traits.contains(&owner.join("::")) {
                    self.visitor_error = Some(format!(
                        "trait-qualified call `{}` in module-qualified caller `{}` is unsupported by the active call graph",
                        segments.join("::"),
                        self.caller_id
                    ));
                    return;
                }
            }
            if segments.first().map(String::as_str) == Some("Self") {
                if let Some(name) = segments.last() {
                    self.self_method_names.push(name.clone());
                }
                segments.clear();
            }
            if segments.len() == 1 && self.bound_values.contains(&segments[0]) {
                segments.clear();
            }
            if !segments.is_empty() {
                match resolve_lexical_import(
                    &self.module,
                    &self.lexical_imports,
                    &segments,
                    self.modules,
                    self.definitions,
                ) {
                    Ok(Some(imported)) => self.paths.push(imported),
                    Ok(None) => self.paths.push(segments),
                    Err(err) => {
                        self.visitor_error = Some(format!(
                            "{err} in module-qualified caller `{}`",
                            self.caller_id
                        ));
                        return;
                    }
                }
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        match inspect_macro_expressions(&node.mac, &self.caller_id) {
            Ok(expressions) => {
                for expression in &expressions {
                    self.visit_expr(expression);
                }
            }
            Err(err) => self.visitor_error = Some(err),
        }
    }

    fn visit_stmt_macro(&mut self, node: &'ast syn::StmtMacro) {
        match inspect_macro_expressions(&node.mac, &self.caller_id) {
            Ok(expressions) => {
                for expression in &expressions {
                    self.visit_expr(expression);
                }
            }
            Err(err) => self.visitor_error = Some(err),
        }
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let name = node.method.to_string();
        if matches!(&*node.receiver, Expr::Path(path) if path.path.is_ident("self")) {
            self.self_method_names.push(name);
        } else {
            let mut known_external_receiver = false;
            let container_wrapper = matches!(name.as_str(), "unwrap" | "expect")
                && self
                    .container_item_expression_type(&Expr::MethodCall(node.clone()))
                    .is_some();
            let owner = if container_wrapper {
                known_external_receiver = true;
                None
            } else {
                match &*node.receiver {
                    Expr::Path(path) => {
                        let identifier = path.path.get_ident().map(ToString::to_string);
                        if identifier
                            .as_ref()
                            .is_some_and(|name| self.known_external_locals.contains(name))
                        {
                            known_external_receiver = true;
                        }
                        identifier.and_then(|name| self.local_types.get(&name).cloned())
                    }
                    Expr::Call(call) => call_return_type(
                        call,
                        &self.module,
                        self.modules,
                        self.function_return_types,
                    ),
                    Expr::Field(field) => {
                        let base_owner = match &*field.base {
                            Expr::Path(path) if path.path.is_ident("self") => {
                                self.callable_receiver_type.clone()
                            }
                            Expr::Path(path) => path.path.get_ident().and_then(|ident| {
                                self.local_types.get(&ident.to_string()).cloned()
                            }),
                            _ => None,
                        };
                        let field_owner = base_owner.as_ref().and_then(|receiver| {
                            let syn::Member::Named(field_name) = &field.member else {
                                return None;
                            };
                            self.struct_fields
                                .get(&(receiver.join("::"), field_name.to_string()))
                        });
                        if field_owner.is_some_and(Option::is_none) {
                            known_external_receiver = true;
                        }
                        field_owner.cloned().flatten()
                    }
                    Expr::MethodCall(inner) => {
                        let inferred =
                            self.inferred_expression_type(&Expr::MethodCall(inner.clone()));
                        if inferred.is_none() {
                            known_external_receiver = !expression_starts_with_local_receiver(
                                &inner.receiver,
                                &self.module,
                                self.modules,
                                self.repository_types,
                                &self.local_types,
                                self.function_return_types,
                                self.callable_receiver_type.as_ref(),
                            );
                        }
                        inferred
                    }
                    _ => None,
                }
            };
            if let Some(mut owner) = owner {
                owner.push(name);
                self.typed_method_paths.push(owner);
            } else if !known_external_receiver {
                self.unresolved_method_names.push(name);
            }
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        self.visit_assignment(node);
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        let names = pattern_identifiers(&node.pat);
        let name = local_identifier(&node.pat);
        let resolved = local_type(
            node,
            &self.module,
            self.modules,
            self.repository_types,
            self.function_return_types,
            self.function_try_return_types,
        );
        let definitely_external = node.init.as_ref().is_some_and(|init| {
            expression_starts_with_known_external_receiver(
                &init.expr,
                &self.local_types,
                &self.known_external_locals,
                self.repository_types,
            )
        });
        if let Some(init) = &node.init {
            self.visit_expr(&init.expr);
            if let Some((_, diverge)) = &init.diverge {
                self.visit_expr(diverge);
            }
        }
        for binding in &names {
            self.local_types.remove(binding);
            self.known_external_locals.remove(binding);
            self.iterable_item_types.remove(binding);
            self.bound_values.insert(binding.clone());
        }
        let Some(name) = name else {
            return;
        };
        if let Pat::Type(typed) = &node.pat
            && let Some(item_owner) = iterable_item_type(
                typed.ty.as_ref(),
                &self.module,
                self.modules,
                self.repository_types,
            )
        {
            self.iterable_item_types.insert(name.clone(), item_owner);
        }
        if let Some((_, owner)) = resolved {
            self.known_external_locals.remove(&name);
            self.local_types.insert(name, owner);
        } else {
            self.local_types.remove(&name);
            if definitely_external {
                self.known_external_locals.insert(name);
            } else {
                self.known_external_locals.remove(&name);
            }
        }
    }
}
