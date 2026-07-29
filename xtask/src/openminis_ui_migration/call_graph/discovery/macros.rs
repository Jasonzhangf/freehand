use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::Expr;
use syn::visit::{self, Visit};

pub(super) fn inspect_macro_expressions(
    call: &syn::Macro,
    caller_id: &str,
) -> Result<Vec<Expr>, String> {
    if call.path.is_ident("matches") {
        return inspect_matches_macro(call, caller_id);
    }
    let mut expressions = Vec::new();
    collect_macro_expressions(call.tokens.clone(), &mut expressions).map_err(|_| {
        format!(
            "Rust call graph cannot inspect opaque macro `{}` in module-qualified caller `{caller_id}`; unexpanded macros may hide local call edges",
            macro_path(&call.path)
        )
    })?;
    Ok(expressions)
}

fn inspect_matches_macro(call: &syn::Macro, caller_id: &str) -> Result<Vec<Expr>, String> {
    let tokens = call.tokens.clone().into_iter().collect::<Vec<_>>();
    let Some(comma) = tokens
        .iter()
        .position(|token| matches!(token, TokenTree::Punct(punct) if punct.as_char() == ','))
    else {
        return Err(format!(
            "Rust call graph cannot inspect malformed `matches!` in module-qualified caller `{caller_id}`"
        ));
    };
    let mut expressions = vec![syn::parse2(tokens[..comma].iter().cloned().collect()).map_err(
        |_| {
            format!(
                "Rust call graph cannot inspect `matches!` value in module-qualified caller `{caller_id}`"
            )
        },
    )?];
    if let Some(guard) = tokens[comma + 1..]
        .iter()
        .position(|token| matches!(token, TokenTree::Ident(ident) if ident == "if"))
    {
        let guard_tokens = tokens[comma + guard + 2..].iter().cloned().collect();
        expressions.push(syn::parse2(guard_tokens).map_err(|_| {
            format!(
                "Rust call graph cannot inspect `matches!` guard in module-qualified caller `{caller_id}`"
            )
        })?);
    }
    Ok(expressions)
}

fn collect_macro_expressions(tokens: TokenStream, expressions: &mut Vec<Expr>) -> Result<(), ()> {
    let mut segment = TokenStream::new();
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].clone();
        let is_path_colon = matches!(&token, TokenTree::Punct(punct) if punct.as_char() == ':')
            && (matches!(tokens.get(index.wrapping_sub(1)), Some(TokenTree::Punct(punct)) if punct.as_char() == ':')
                || matches!(tokens.get(index + 1), Some(TokenTree::Punct(punct)) if punct.as_char() == ':'));
        let separator = matches!(&token, TokenTree::Punct(punct) if matches!(punct.as_char(), ',' | ':' | ';' | '='))
            && !is_path_colon;
        if separator {
            collect_macro_segment(segment, expressions)?;
            segment = TokenStream::new();
        } else {
            segment.extend([token]);
        }
        index += 1;
    }
    collect_macro_segment(segment, expressions)
}

fn collect_macro_segment(segment: TokenStream, expressions: &mut Vec<Expr>) -> Result<(), ()> {
    if segment.is_empty() {
        return Ok(());
    }
    if let Ok(expression) = syn::parse2::<Expr>(segment.clone()) {
        expressions.push(expression);
        return Ok(());
    }
    let mut direct_call_like = false;
    let mut callable_prefix = false;
    for token in segment {
        match token {
            TokenTree::Ident(_) => callable_prefix = true,
            TokenTree::Punct(punct) => {
                callable_prefix = punct.as_char() == '>';
            }
            TokenTree::Literal(_) => callable_prefix = false,
            TokenTree::Group(group) => {
                if group.delimiter() == Delimiter::Parenthesis && callable_prefix {
                    direct_call_like = true;
                }
                collect_macro_expressions(group.stream(), expressions)?;
                callable_prefix = false;
            }
        }
    }
    (!direct_call_like).then_some(()).ok_or(())
}

fn macro_path(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn reject_unexpanded_call_sources(
    syntax: &syn::File,
    owner: &str,
) -> Result<(), String> {
    let mut visitor = UnexpandedMacroVisitor { owner, error: None };
    visitor.visit_file(syntax);
    visitor.error.map_or(Ok(()), Err)
}

struct UnexpandedMacroVisitor<'a> {
    owner: &'a str,
    error: Option<String>,
}

impl UnexpandedMacroVisitor<'_> {
    fn reject(&mut self, kind: &str, path: &syn::Path) {
        if self.error.is_some() {
            return;
        }
        let name = macro_path(path);
        self.error = Some(format!(
            "Rust call graph cannot inspect {kind} `{name}!` in `{}`; unexpanded macros may hide local call edges",
            self.owner
        ));
    }
}

impl<'ast> Visit<'ast> for UnexpandedMacroVisitor<'_> {
    fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
        self.reject("item macro", &node.mac.path);
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        if node.mac.path.is_ident("include") {
            self.reject("expression macro", &node.mac.path);
            return;
        }
        visit::visit_expr_macro(self, node);
    }

    fn visit_stmt_macro(&mut self, node: &'ast syn::StmtMacro) {
        if node.mac.path.is_ident("include") {
            self.reject("statement macro", &node.mac.path);
            return;
        }
        visit::visit_stmt_macro(self, node);
    }
}
