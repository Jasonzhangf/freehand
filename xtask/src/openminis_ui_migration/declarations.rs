use super::*;
use syn::visit::{self, Visit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DeclarationOccurrence {
    pub file: String,
    pub name: String,
    pub scope: String,
    pub ordinal: usize,
}

pub(super) fn declared_symbols(
    path: &Path,
    source: &str,
) -> Result<Vec<DeclarationOccurrence>, String> {
    match path.extension().and_then(|value| value.to_str()) {
        Some("rs") => rust_declarations(path, source),
        Some("swift") => swift_declarations(path, source),
        Some("js" | "jsx" | "mjs") => syntax_declarations(
            path,
            source,
            tree_sitter_javascript::LANGUAGE.into(),
            DeclarationLanguage::JavaScript,
        ),
        Some("ts") => syntax_declarations(
            path,
            source,
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            DeclarationLanguage::JavaScript,
        ),
        Some("tsx") => syntax_declarations(
            path,
            source,
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            DeclarationLanguage::JavaScript,
        ),
        Some("kt" | "kts") => syntax_declarations(
            path,
            source,
            tree_sitter_kotlin_ng::LANGUAGE.into(),
            DeclarationLanguage::Kotlin,
        ),
        extension => Err(format!(
            "unsupported declaration language for {} ({extension:?})",
            path.display()
        )),
    }
}

pub(super) fn is_declaration_source_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("rs" | "swift" | "js" | "jsx" | "mjs" | "ts" | "tsx" | "kt" | "kts")
    )
}

fn rust_declarations(path: &Path, source: &str) -> Result<Vec<DeclarationOccurrence>, String> {
    #[derive(Default)]
    struct DeclarationVisitor {
        file: String,
        occurrences: Vec<DeclarationOccurrence>,
        scope: Vec<String>,
        execution_depth: usize,
    }

    impl DeclarationVisitor {
        fn record_occurrence(&mut self, name: String, kind: &str) {
            self.occurrences.push(DeclarationOccurrence {
                file: self.file.clone(),
                name,
                scope: if self.scope.is_empty() {
                    kind.to_owned()
                } else {
                    format!("{}::{kind}", self.scope.join("::"))
                },
                ordinal: self.occurrences.len(),
            });
        }
    }

    impl<'ast> Visit<'ast> for DeclarationVisitor {
        fn visit_item(&mut self, item: &'ast syn::Item) {
            if self.execution_depth > 0 {
                return;
            }
            let declaration = match item {
                syn::Item::Const(value) => Some((value.ident.to_string(), "const")),
                syn::Item::Enum(value) => Some((value.ident.to_string(), "enum")),
                syn::Item::Fn(value) => Some((value.sig.ident.to_string(), "fn")),
                syn::Item::Mod(value) => Some((value.ident.to_string(), "mod")),
                syn::Item::Static(value) => Some((value.ident.to_string(), "static")),
                syn::Item::Struct(value) => Some((value.ident.to_string(), "struct")),
                syn::Item::Trait(value) => Some((value.ident.to_string(), "trait")),
                syn::Item::TraitAlias(value) => Some((value.ident.to_string(), "trait_alias")),
                syn::Item::Type(value) => Some((value.ident.to_string(), "type")),
                syn::Item::Union(value) => Some((value.ident.to_string(), "union")),
                _ => None,
            };
            if let Some((name, kind)) = declaration {
                self.record_occurrence(name, kind);
            }
            visit::visit_item(self, item);
        }

        fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
            let scope = item
                .trait_
                .as_ref()
                .and_then(|(_, path, _)| path.segments.last())
                .map(|segment| format!("impl trait {}", segment.ident))
                .unwrap_or_else(|| format!("impl#{}", self.occurrences.len()));
            self.scope.push(scope);
            visit::visit_item_impl(self, item);
            self.scope.pop();
        }

        fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
            self.scope.push(format!("trait {}", item.ident));
            visit::visit_item_trait(self, item);
            self.scope.pop();
        }

        fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
            self.record_occurrence(item.sig.ident.to_string(), "fn");
            visit::visit_impl_item_fn(self, item);
        }

        fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
            self.record_occurrence(item.sig.ident.to_string(), "fn");
            visit::visit_trait_item_fn(self, item);
        }

        fn visit_block(&mut self, block: &'ast syn::Block) {
            self.execution_depth += 1;
            visit::visit_block(self, block);
            self.execution_depth -= 1;
        }
    }

    let syntax =
        syn::parse_file(source).map_err(|err| format!("parse Rust declarations: {err}"))?;
    let mut visitor = DeclarationVisitor {
        file: path.to_string_lossy().into_owned(),
        ..DeclarationVisitor::default()
    };
    visitor.visit_file(&syntax);
    Ok(visitor.occurrences)
}

#[derive(Clone, Copy)]
enum DeclarationLanguage {
    JavaScript,
    Kotlin,
}

fn syntax_declarations(
    path: &Path,
    source: &str,
    language: tree_sitter::Language,
    declaration_language: DeclarationLanguage,
) -> Result<Vec<DeclarationOccurrence>, String> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|err| format!("load declaration grammar for {}: {err}", path.display()))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| format!("parse declarations in {} returned no tree", path.display()))?;
    if tree.root_node().has_error() {
        return Err(format!(
            "parse declarations in {} produced a syntax-error tree",
            path.display()
        ));
    }
    let mut output = Vec::new();
    collect_syntax_declarations(
        tree.root_node(),
        path,
        source.as_bytes(),
        declaration_language,
        &mut output,
    )?;
    Ok(output)
}

fn collect_syntax_declarations(
    node: tree_sitter::Node<'_>,
    path: &Path,
    source: &[u8],
    language: DeclarationLanguage,
    output: &mut Vec<DeclarationOccurrence>,
) -> Result<(), String> {
    let kind = node.kind();
    let declaration = match language {
        DeclarationLanguage::JavaScript => {
            matches!(
                kind,
                "class_declaration"
                    | "function_declaration"
                    | "generator_function_declaration"
                    | "interface_declaration"
                    | "type_alias_declaration"
                    | "variable_declarator"
            )
        }
        DeclarationLanguage::Kotlin => matches!(
            kind,
            "class_declaration"
                | "function_declaration"
                | "object_declaration"
                | "type_alias"
                | "variable_declaration"
        ),
    };
    if declaration && has_no_local_scope_ancestor(node) {
        let name_node = node
            .child_by_field_name("name")
            .or_else(|| {
                (matches!(language, DeclarationLanguage::Kotlin) && kind == "variable_declaration")
                    .then(|| first_named_child_of_kind(node, "identifier"))
                    .flatten()
            })
            .ok_or_else(|| {
                format!(
                    "declaration AST node `{kind}` in {} has no supported name",
                    path.display()
                )
            })?;
        let name = name_node
            .utf8_text(source)
            .map_err(|err| format!("read declaration name in {}: {err}", path.display()))?;
        if !name_node.has_error() && is_identifier(name) {
            output.push(DeclarationOccurrence {
                file: path.to_string_lossy().into_owned(),
                name: name.to_owned(),
                scope: kind.to_owned(),
                ordinal: output.len(),
            });
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_syntax_declarations(child, path, source, language, output)?;
    }
    Ok(())
}

fn has_no_local_scope_ancestor(node: tree_sitter::Node<'_>) -> bool {
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if matches!(
            ancestor.kind(),
            "statement_block"
                | "function_body"
                | "block"
                | "control_structure_body"
                | "lambda_literal"
                | "anonymous_function"
                | "for_statement"
                | "for_in_statement"
                | "if_statement"
                | "while_statement"
                | "do_statement"
                | "switch_statement"
                | "switch_case"
                | "catch_clause"
                | "try_statement"
                | "when_expression"
        ) {
            return false;
        }
        parent = ancestor.parent();
    }
    true
}

fn swift_declarations(path: &Path, source: &str) -> Result<Vec<DeclarationOccurrence>, String> {
    let temp_path = create_swift_parse_input(source)?;
    let output = Command::new("swiftc")
        .args(["-frontend", "-dump-parse", "-enable-bare-slash-regex"])
        .arg(&temp_path)
        .output()
        .map_err(|err| format!("run Swift parser for {}: {err}", path.display()));
    let remove_result = fs::remove_file(&temp_path);
    let output = output?;
    remove_result.map_err(|err| {
        format!(
            "remove Swift parser input {} for {}: {err}",
            temp_path.display(),
            path.display()
        )
    })?;
    if !output.status.success() {
        return Err(format!(
            "Swift parser rejected {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let ast = String::from_utf8(output.stdout).map_err(|err| {
        format!(
            "Swift parser output for {} is not UTF-8: {err}",
            path.display()
        )
    })?;
    let mut declarations = Vec::new();
    let mut scopes = Vec::<(usize, String)>::new();
    for line in ast.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        while scopes.last().is_some_and(|(depth, _)| *depth >= indent) {
            scopes.pop();
        }
        let Some(kind) = swift_ast_declaration_kind(trimmed) else {
            if let Some(node_kind) = swift_ast_node_kind(trimmed) {
                scopes.push((indent, node_kind.to_owned()));
            }
            continue;
        };
        let name = first_quoted_value(trimmed).ok_or_else(|| {
            format!(
                "Swift AST declaration `{kind}` in {} has no name",
                path.display()
            )
        })?;
        let name = name.split('(').next().unwrap_or(name);
        let inside_execution_scope = scopes
            .iter()
            .any(|(_, scope)| swift_ast_execution_scope_kind(scope));
        if is_identifier(name) && !inside_execution_scope {
            declarations.push(DeclarationOccurrence {
                file: path.to_string_lossy().into_owned(),
                name: name.to_owned(),
                scope: kind.to_owned(),
                ordinal: declarations.len(),
            });
        }
        scopes.push((indent, kind.to_owned()));
    }
    Ok(declarations)
}

fn create_swift_parse_input(source: &str) -> Result<PathBuf, String> {
    for attempt in 0..100 {
        let path = std::env::temp_dir().join(format!(
            "freehand-openminis-swift-{}-{attempt}.swift",
            std::process::id()
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write;
                file.write_all(source.as_bytes())
                    .map_err(|err| format!("write Swift parser input {}: {err}", path.display()))?;
                return Ok(path);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => {
                return Err(format!(
                    "create Swift parser input {}: {err}",
                    path.display()
                ));
            }
        }
    }
    Err("cannot allocate unique Swift parser input after 100 attempts".to_owned())
}

fn swift_ast_declaration_kind(line: &str) -> Option<&str> {
    [
        "actor_decl",
        "class_decl",
        "enum_decl",
        "func_decl",
        "protocol_decl",
        "struct_decl",
        "typealias_decl",
        "var_decl",
    ]
    .into_iter()
    .find(|kind| line.starts_with(&format!("({kind} ")))
}

fn swift_ast_node_kind(line: &str) -> Option<&str> {
    let node = line.strip_prefix('(')?;
    let end = node.find([' ', ')'])?;
    Some(&node[..end])
}

fn swift_ast_execution_scope_kind(kind: &str) -> bool {
    matches!(
        kind,
        "brace_stmt"
            | "closure_expr"
            | "constructor_decl"
            | "destructor_decl"
            | "func_decl"
            | "accessor_decl"
    )
}

fn first_quoted_value(line: &str) -> Option<&str> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(&line[start..end])
}

fn first_named_child_of_kind<'tree>(
    node: tree_sitter::Node<'tree>,
    kind: &str,
) -> Option<tree_sitter::Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_alphanumeric())
}
