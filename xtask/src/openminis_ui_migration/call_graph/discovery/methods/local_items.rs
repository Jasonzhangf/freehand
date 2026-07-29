use super::*;
use syn::visit::{self, Visit};

struct LocalInitializer<'a> {
    id: String,
    attrs: &'a [syn::Attribute],
    expression: &'a syn::Expr,
}

struct NestedLocalItemCollector<'a> {
    active_cfg: &'a BTreeSet<String>,
    lexical_owner: &'a str,
    depth: usize,
    nested_initializer_ordinal: usize,
    initializers: Vec<LocalInitializer<'a>>,
    error: Option<String>,
}

impl<'ast> Visit<'ast> for NestedLocalItemCollector<'ast> {
    fn visit_stmt(&mut self, statement: &'ast Stmt) {
        if self.error.is_some() {
            return;
        }
        let attrs = match statement {
            Stmt::Local(value) => &value.attrs,
            Stmt::Item(value) => item_attrs(value).unwrap_or(&[]),
            Stmt::Expr(value, _) => visitor::expression_attrs(value),
            Stmt::Macro(value) => &value.attrs,
        };
        match cfg_enabled(
            attrs,
            self.active_cfg,
            &format!(
                "local item statement in module-qualified caller `{}`",
                self.lexical_owner
            ),
        ) {
            Ok(true) => visit::visit_stmt(self, statement),
            Ok(false) => {}
            Err(err) => self.error = Some(err),
        }
    }

    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        if self.error.is_some() {
            return;
        }
        match cfg_enabled(
            visitor::expression_attrs(expression),
            self.active_cfg,
            &format!(
                "local item expression in module-qualified caller `{}`",
                self.lexical_owner
            ),
        ) {
            Ok(true) => visit::visit_expr(self, expression),
            Ok(false) => {}
            Err(err) => self.error = Some(err),
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.depth += 1;
        for statement in &block.stmts {
            self.visit_stmt(statement);
        }
        self.depth -= 1;
    }

    fn visit_item_impl(&mut self, _item: &'ast syn::ItemImpl) {
        if self.depth > 0 {
            self.error = Some(format!(
                "nested impl in module-qualified caller `{}` is unsupported because it has no stable nested declaration identity",
                self.lexical_owner
            ));
        }
    }

    fn visit_item_trait(&mut self, _item: &'ast syn::ItemTrait) {
        if self.depth > 0 {
            self.error = Some(format!(
                "nested trait in module-qualified caller `{}` is unsupported because it has no stable nested declaration identity",
                self.lexical_owner
            ));
        }
    }

    fn visit_item_fn(&mut self, _item: &'ast syn::ItemFn) {}

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.collect_initializer(&item.ident, &item.attrs, &item.expr);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.collect_initializer(&item.ident, &item.attrs, &item.expr);
    }
}

impl<'a> NestedLocalItemCollector<'a> {
    fn collect_initializer(
        &mut self,
        ident: &syn::Ident,
        attrs: &'a [syn::Attribute],
        expression: &'a syn::Expr,
    ) {
        let item_owner = if self.depth == 0 {
            format!("{}::{ident}", self.lexical_owner)
        } else {
            let ordinal = self.nested_initializer_ordinal;
            self.nested_initializer_ordinal += 1;
            format!("{}::__block_item_{ordinal}::{ident}", self.lexical_owner)
        };
        self.initializers.push(LocalInitializer {
            id: format!("{item_owner}::__initializer"),
            attrs,
            expression,
        });
    }
}

pub(super) fn collect_local_items_impl(
    statements: &[Stmt],
    enclosing_module: &[String],
    lexical_owner: &str,
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let mut nested = NestedLocalItemCollector {
        active_cfg: state.active_cfg,
        lexical_owner,
        depth: 0,
        nested_initializer_ordinal: 0,
        initializers: Vec::new(),
        error: None,
    };
    for statement in statements {
        nested.visit_stmt(statement);
    }
    if let Some(err) = nested.error {
        return Err(err);
    }

    let scope = lexical_owner
        .split("::")
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    state.modules.entry(lexical_owner.to_owned()).or_default();
    let items = statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Item(item) => Some(item),
            _ => None,
        })
        .filter_map(|item| {
            let attrs = item_attrs(item).unwrap_or(&[]);
            match cfg_enabled(
                attrs,
                state.active_cfg,
                &format!("block item in module-qualified caller `{lexical_owner}`"),
            ) {
                Ok(true) => Some(Ok(item)),
                Ok(false) => None,
                Err(err) => Some(Err(err)),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    index_local_types(&items, lexical_owner, state);
    index_local_struct_fields(&items, lexical_owner, &scope, state);
    for item in items {
        match item {
            Item::Impl(item) => {
                collect_impl(item, &scope, enclosing_module, owner, inherited_test, state)?
            }
            Item::Trait(item) => {
                collect_trait(item, &scope, enclosing_module, owner, inherited_test, state)?
            }
            _ => {}
        }
    }
    for initializer in nested.initializers {
        visitor::collect_initializer_impl(
            InitializerSource {
                id: initializer.id,
                attrs: initializer.attrs,
                expression: initializer.expression,
                module: enclosing_module,
                call_scope: &scope,
                owner,
                method_owner: None,
                receiver_type: None,
                inherited_test,
            },
            state,
        )?;
    }
    Ok(())
}

fn index_local_types(items: &[&Item], lexical_owner: &str, state: &mut DiscoveryState<'_>) {
    for item in items {
        let name = match item {
            Item::Struct(value) => Some(&value.ident),
            Item::Enum(value) => Some(&value.ident),
            Item::Union(value) => Some(&value.ident),
            Item::Type(value) => Some(&value.ident),
            _ => None,
        };
        if let Some(name) = name {
            state.local_types.insert(format!("{lexical_owner}::{name}"));
        }
        if let Item::Trait(value) = item {
            let trait_id = format!("{lexical_owner}::{}", value.ident);
            state.local_types.insert(trait_id.clone());
            state.local_traits.insert(trait_id);
        }
        if let Item::Use(item_use) = item {
            let mut imports = Vec::new();
            flatten_use_tree(&item_use.tree, Vec::new(), &mut imports);
            state
                .modules
                .get_mut(lexical_owner)
                .expect("lexical module initialized")
                .imports
                .extend(imports);
        }
    }
}

fn index_local_struct_fields(
    items: &[&Item],
    lexical_owner: &str,
    scope: &[String],
    state: &mut DiscoveryState<'_>,
) {
    for item in items {
        let Item::Struct(value) = item else {
            continue;
        };
        let receiver = format!("{lexical_owner}::{}", value.ident);
        for field in &value.fields {
            let Some(name) = &field.ident else {
                continue;
            };
            let owner = type_identity(&field.ty, scope, state.modules, state.local_types)
                .filter(|owner| state.local_types.contains(&owner.join("::")));
            state
                .struct_fields
                .insert((receiver.clone(), name.to_string()), owner);
        }
    }
}
