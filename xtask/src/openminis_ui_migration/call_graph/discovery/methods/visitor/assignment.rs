use super::*;

impl FunctionCallVisitor<'_> {
    pub(super) fn visit_assignment(&mut self, node: &syn::ExprAssign) {
        self.visit_expr(&node.left);
        self.visit_expr(&node.right);
        let Expr::Path(path) = node.left.as_ref() else {
            return;
        };
        let Some(name) = path.path.get_ident().map(ToString::to_string) else {
            return;
        };
        if !self.bound_values.contains(&name) {
            return;
        }
        let owner = self.inferred_expression_type(&node.right);
        let definitely_external = expression_starts_with_known_external_receiver(
            &node.right,
            &self.local_types,
            &self.known_external_locals,
            self.repository_types,
        );
        self.local_types.remove(&name);
        self.known_external_locals.remove(&name);
        self.iterable_item_types.remove(&name);
        if let Some(owner) = owner {
            if self.repository_types.contains(&owner.join("::")) {
                self.local_types.insert(name, owner);
            } else {
                self.known_external_locals.insert(name);
            }
        } else if definitely_external {
            self.known_external_locals.insert(name);
        }
    }
}
