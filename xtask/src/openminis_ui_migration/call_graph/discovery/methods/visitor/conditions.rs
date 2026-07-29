use super::*;

pub(in crate::openminis_ui_migration::call_graph::discovery::methods) fn expression_attrs(
    expression: &Expr,
) -> &[syn::Attribute] {
    match expression {
        Expr::Array(value) => &value.attrs,
        Expr::Assign(value) => &value.attrs,
        Expr::Async(value) => &value.attrs,
        Expr::Await(value) => &value.attrs,
        Expr::Binary(value) => &value.attrs,
        Expr::Block(value) => &value.attrs,
        Expr::Break(value) => &value.attrs,
        Expr::Call(value) => &value.attrs,
        Expr::Cast(value) => &value.attrs,
        Expr::Closure(value) => &value.attrs,
        Expr::Const(value) => &value.attrs,
        Expr::Continue(value) => &value.attrs,
        Expr::Field(value) => &value.attrs,
        Expr::ForLoop(value) => &value.attrs,
        Expr::Group(value) => &value.attrs,
        Expr::If(value) => &value.attrs,
        Expr::Index(value) => &value.attrs,
        Expr::Infer(value) => &value.attrs,
        Expr::Let(value) => &value.attrs,
        Expr::Lit(value) => &value.attrs,
        Expr::Loop(value) => &value.attrs,
        Expr::Macro(value) => &value.attrs,
        Expr::Match(value) => &value.attrs,
        Expr::MethodCall(value) => &value.attrs,
        Expr::Paren(value) => &value.attrs,
        Expr::Path(value) => &value.attrs,
        Expr::Range(value) => &value.attrs,
        Expr::RawAddr(value) => &value.attrs,
        Expr::Reference(value) => &value.attrs,
        Expr::Repeat(value) => &value.attrs,
        Expr::Return(value) => &value.attrs,
        Expr::Struct(value) => &value.attrs,
        Expr::Try(value) => &value.attrs,
        Expr::TryBlock(value) => &value.attrs,
        Expr::Tuple(value) => &value.attrs,
        Expr::Unary(value) => &value.attrs,
        Expr::Unsafe(value) => &value.attrs,
        Expr::Verbatim(_) => &[],
        Expr::While(value) => &value.attrs,
        Expr::Yield(value) => &value.attrs,
        _ => &[],
    }
}

impl FunctionCallVisitor<'_> {
    pub(super) fn inferred_expression_type(&self, expression: &Expr) -> Option<Vec<String>> {
        let inferred = super::super::receiver::expression_type(
            expression,
            &self.module,
            self.modules,
            self.repository_types,
            self.function_return_types,
            self.function_try_return_types,
        );
        inferred.or_else(|| self.container_item_expression_type(expression))
    }

    pub(super) fn container_item_expression_type(&self, expression: &Expr) -> Option<Vec<String>> {
        let Expr::MethodCall(call) = expression else {
            return None;
        };
        let method = call.method.to_string();
        if matches!(method.as_str(), "unwrap" | "expect") {
            return self.container_item_expression_type(&call.receiver);
        }
        if !matches!(
            method.as_str(),
            "pop"
                | "pop_front"
                | "pop_back"
                | "first"
                | "first_mut"
                | "last"
                | "last_mut"
                | "get"
                | "get_mut"
                | "remove"
                | "swap_remove"
        ) {
            return None;
        }
        let Expr::Path(path) = call.receiver.as_ref() else {
            return None;
        };
        path.path
            .get_ident()
            .and_then(|name| self.iterable_item_types.get(&name.to_string()))
            .cloned()
    }

    pub(super) fn visit_cfg_statement(&mut self, statement: &Stmt) {
        let attrs = match statement {
            Stmt::Local(value) => &value.attrs,
            Stmt::Item(value) => item_attrs(value).unwrap_or(&[]),
            Stmt::Expr(value, _) => expression_attrs(value),
            Stmt::Macro(value) => &value.attrs,
        };
        match cfg_enabled(
            attrs,
            &self.active_cfg,
            &format!("statement in module-qualified caller `{}`", self.caller_id),
        ) {
            Ok(true) => visit::visit_stmt(self, statement),
            Ok(false) => {}
            Err(err) => self.visitor_error = Some(err),
        }
    }

    pub(super) fn visit_lexical_block(&mut self, block: &syn::Block) {
        let outer_types = self.local_types.clone();
        let outer_external_locals = self.known_external_locals.clone();
        let outer_iterable_item_types = self.iterable_item_types.clone();
        let outer_bound_values = self.bound_values.clone();
        let outer_imports = self.lexical_imports.clone();
        if let Err(err) = collect_block_scope_bindings(
            block,
            &mut self.lexical_imports,
            &mut self.bound_values,
            &self.active_cfg,
            &self.caller_id,
        ) {
            self.visitor_error = Some(err);
            return;
        }
        visit::visit_block(self, block);
        self.local_types = outer_types;
        self.known_external_locals = outer_external_locals;
        self.iterable_item_types = outer_iterable_item_types;
        self.bound_values = outer_bound_values;
        self.lexical_imports = outer_imports;
    }

    pub(super) fn visit_condition(&mut self, condition: &Expr) {
        match condition {
            Expr::Binary(binary) if matches!(binary.op, syn::BinOp::And(_)) => {
                self.visit_condition(&binary.left);
                self.visit_condition(&binary.right);
            }
            Expr::Let(binding) => {
                self.visit_expr(&binding.expr);
                for name in pattern_identifiers(&binding.pat) {
                    self.local_types.remove(&name);
                    self.known_external_locals.remove(&name);
                    self.iterable_item_types.remove(&name);
                    self.bound_values.insert(name);
                }
            }
            Expr::Group(group) => self.visit_condition(&group.expr),
            Expr::Paren(paren) => self.visit_condition(&paren.expr),
            _ => self.visit_expr(condition),
        }
    }
}

impl<'a> FunctionCallVisitor<'a> {
    pub(super) fn from_source(source: &CallableSource<'_>, state: &'a DiscoveryState<'_>) -> Self {
        let mut callable_cfg = state.active_cfg.clone();
        if source.is_test {
            callable_cfg.insert("test".to_owned());
        }
        Self {
            paths: Vec::new(),
            typed_method_paths: Vec::new(),
            unresolved_method_names: Vec::new(),
            self_method_names: Vec::new(),
            visitor_error: None,
            active_cfg: callable_cfg,
            caller_id: source.id.clone(),
            local_types: signature_local_types(
                source.sig,
                source.module,
                state.modules,
                state.local_types,
            ),
            known_external_locals: signature_external_locals(source.sig),
            iterable_item_types: signature_iterable_item_types(
                source.sig,
                source.module,
                state.modules,
                state.local_types,
            ),
            bound_values: signature_binding_names(source.sig),
            lexical_imports: Vec::new(),
            module: source.module.to_vec(),
            modules: state.modules,
            definitions: state.definitions,
            repository_types: state.local_types,
            repository_traits: state.local_traits,
            function_return_types: state.function_return_types,
            function_try_return_types: state.function_try_return_types,
            callable_receiver_type: source.receiver_type.clone(),
            struct_fields: state.struct_fields,
        }
    }

    pub(super) fn from_initializer(
        caller_id: String,
        module: &[String],
        is_test: bool,
        state: &'a DiscoveryState<'_>,
    ) -> Self {
        let mut active_cfg = state.active_cfg.clone();
        if is_test {
            active_cfg.insert("test".to_owned());
        }
        Self {
            paths: Vec::new(),
            typed_method_paths: Vec::new(),
            unresolved_method_names: Vec::new(),
            self_method_names: Vec::new(),
            visitor_error: None,
            active_cfg,
            caller_id,
            local_types: BTreeMap::new(),
            known_external_locals: BTreeSet::new(),
            iterable_item_types: BTreeMap::new(),
            bound_values: BTreeSet::new(),
            lexical_imports: Vec::new(),
            module: module.to_vec(),
            modules: state.modules,
            definitions: state.definitions,
            repository_types: state.local_types,
            repository_traits: state.local_traits,
            function_return_types: state.function_return_types,
            function_try_return_types: state.function_try_return_types,
            callable_receiver_type: None,
            struct_fields: state.struct_fields,
        }
    }
}
