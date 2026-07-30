use super::*;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, Meta, Token};

pub(super) fn active_rust_cfg() -> Result<BTreeSet<String>, String> {
    let output = Command::new("rustc")
        .args(["--print", "cfg"])
        .output()
        .map_err(|err| format!("run rustc --print cfg: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "rustc --print cfg failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let cfg = String::from_utf8(output.stdout)
        .map_err(|err| format!("rustc --print cfg emitted non-UTF8 output: {err}"))?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();
    Ok(cfg)
}

pub(super) fn active_in_cfg_projection(
    attrs: &[syn::Attribute],
    context: &str,
    active_cfg: &BTreeSet<String>,
) -> Result<bool, String> {
    let current = cfg_enabled(attrs, active_cfg, context)?;
    let mut alternate = active_cfg.clone();
    if !alternate.remove("test") {
        alternate.insert("test".to_owned());
    }
    if current || cfg_enabled(attrs, &alternate, context)? {
        return Ok(current);
    }
    Err(format!(
        "{context} is disabled by cfg and cannot enter active call truth"
    ))
}

pub(super) fn cfg_enabled(
    attrs: &[syn::Attribute],
    active_cfg: &BTreeSet<String>,
    context: &str,
) -> Result<bool, String> {
    for attr in attrs {
        if attr.path().is_ident("cfg_attr") {
            return Err(format!("{context} uses unsupported cfg_attr"));
        }
        if !attr.path().is_ident("cfg") {
            continue;
        }
        let predicate = attr
            .parse_args::<Meta>()
            .map_err(|err| format!("parse cfg for {context}: {err}"))?;
        if !evaluate_cfg(&predicate, active_cfg)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn evaluate_cfg(meta: &Meta, active_cfg: &BTreeSet<String>) -> Result<bool, String> {
    match meta {
        Meta::Path(path) => Ok(active_cfg.contains(&path_to_string(path))),
        Meta::NameValue(value) => {
            let Expr::Lit(lit) = &value.value else {
                return Err("cfg name-value must use a literal".to_owned());
            };
            let syn::Lit::Str(lit) = &lit.lit else {
                return Err("cfg name-value must use a string literal".to_owned());
            };
            Ok(active_cfg.contains(&format!(
                "{}=\"{}\"",
                path_to_string(&value.path),
                lit.value()
            )))
        }
        Meta::List(list) => {
            let nested = Punctuated::<Meta, Token![,]>::parse_terminated
                .parse2(list.tokens.clone())
                .map_err(|err| format!("parse nested cfg predicate: {err}"))?;
            let name = path_to_string(&list.path);
            match name.as_str() {
                "all" => {
                    for item in &nested {
                        if !evaluate_cfg(item, active_cfg)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                "any" => {
                    for item in &nested {
                        if evaluate_cfg(item, active_cfg)? {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                "not" if nested.len() == 1 => Ok(!evaluate_cfg(&nested[0], active_cfg)?),
                _ => Err(format!("unsupported cfg operator `{name}`")),
            }
        }
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}
