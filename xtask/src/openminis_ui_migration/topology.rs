use super::*;

pub(super) fn verify_openminis_ui_tree_topology(
    root: &Path,
    manifest: &serde_json::Map<String, Value>,
    node_ids: &BTreeSet<String>,
) -> Result<(), String> {
    let entrypoint = required_string(manifest, "entrypoint_node_id", "OpenMinis UI migration")?;
    if entrypoint != "foundation.root" || !node_ids.contains(entrypoint) {
        return Err(format!(
            "OpenMinis UI migration has invalid entrypoint `{entrypoint}`"
        ));
    }

    let edges = manifest
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| "OpenMinis UI migration manifest missing edges".to_owned())?;
    let mut manifest_edge_pairs = BTreeSet::new();
    let mut manifest_edges = BTreeSet::new();
    let mut edge_ids = BTreeSet::new();
    for edge in edges {
        let edge = edge
            .as_object()
            .ok_or_else(|| "OpenMinis UI migration edges must contain objects".to_owned())?;
        let edge_id = required_string(edge, "edge_id", "OpenMinis UI migration edge")?;
        let from = required_string(edge, "from_node_id", edge_id)?;
        let to = required_string(edge, "to_node_id", edge_id)?;
        let semantic = required_string(edge, "semantic", edge_id)?;
        if !edge_ids.insert(edge_id.to_owned()) {
            return Err(format!(
                "OpenMinis UI migration has duplicate edge_id `{edge_id}`"
            ));
        }
        if !node_ids.contains(from) || !node_ids.contains(to) {
            return Err(format!(
                "OpenMinis UI migration edge `{edge_id}` references unknown node `{from}` -> `{to}`"
            ));
        }
        if !manifest_edge_pairs.insert((from.to_owned(), to.to_owned())) {
            return Err(format!(
                "OpenMinis UI migration has duplicate edge `{from}` -> `{to}`"
            ));
        }
        manifest_edges.insert((
            edge_id.to_owned(),
            from.to_owned(),
            to.to_owned(),
            semantic.to_owned(),
        ));
    }
    if manifest_edges.is_empty() {
        return Err("OpenMinis UI migration topology must contain edges".to_owned());
    }
    verify_openminis_ui_reachability(entrypoint, node_ids, &manifest_edge_pairs)?;
    verify_openminis_ui_node_route_edges(manifest, &manifest_edges)?;

    let markdown = fs::read_to_string(root.join("docs/migrations/openminis-ui/ui-tree.md"))
        .map_err(|err| format!("read OpenMinis UI migration tree: {err}"))?;
    let (markdown_nodes, markdown_edge_pairs) = parse_openminis_ui_mermaid_tree(&markdown)?;
    if markdown_nodes != *node_ids {
        return Err(format!(
            "OpenMinis UI migration Mermaid node set drift: machine={node_ids:?}, human={markdown_nodes:?}"
        ));
    }
    if markdown_edge_pairs != manifest_edge_pairs {
        return Err(format!(
            "OpenMinis UI migration Mermaid edge set drift: machine={manifest_edge_pairs:?}, human={markdown_edge_pairs:?}"
        ));
    }

    let return_paths = manifest
        .get("return_paths")
        .and_then(Value::as_array)
        .ok_or_else(|| "OpenMinis UI migration manifest missing return_paths".to_owned())?;
    if return_paths.is_empty() {
        return Err("OpenMinis UI migration return_paths must not be empty".to_owned());
    }
    let mut manifest_returns = BTreeSet::new();
    for path in return_paths {
        let path = path
            .as_object()
            .ok_or_else(|| "OpenMinis UI migration return_paths must contain objects".to_owned())?;
        let from = required_string(path, "from_node_id", "OpenMinis UI return path")?;
        let to = required_string(path, "to_node_id", "OpenMinis UI return path")?;
        let semantic = required_string(path, "semantic", "OpenMinis UI return path")?;
        if !node_ids.contains(from) || to != entrypoint {
            return Err(format!(
                "OpenMinis UI migration invalid return path `{from}` -> `{to}`"
            ));
        }
        if !manifest_returns.insert((from.to_owned(), to.to_owned(), semantic.to_owned())) {
            return Err(format!(
                "OpenMinis UI migration has duplicate return path `{from}` -> `{to}` / `{semantic}`"
            ));
        }
    }

    let (human_entrypoint, human_edges, human_returns) =
        parse_openminis_ui_registered_paths(&markdown)?;
    if human_entrypoint != entrypoint {
        return Err(format!(
            "OpenMinis UI migration human entrypoint drift: machine=`{entrypoint}`, human=`{human_entrypoint}`"
        ));
    }
    if human_edges != manifest_edges {
        return Err(format!(
            "OpenMinis UI migration human forward-edge registry drift: machine={manifest_edges:?}, human={human_edges:?}"
        ));
    }
    if human_returns != manifest_returns {
        return Err(format!(
            "OpenMinis UI migration human return-path registry drift: machine={manifest_returns:?}, human={human_returns:?}"
        ));
    }
    Ok(())
}

fn verify_openminis_ui_reachability(
    entrypoint: &str,
    node_ids: &BTreeSet<String>,
    edge_pairs: &BTreeSet<(String, String)>,
) -> Result<(), String> {
    let mut reachable = BTreeSet::from([entrypoint.to_owned()]);
    loop {
        let before = reachable.len();
        for (from, to) in edge_pairs {
            if reachable.contains(from) {
                reachable.insert(to.clone());
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    let unreachable = node_ids
        .difference(&reachable)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !unreachable.is_empty() {
        return Err(format!(
            "OpenMinis UI migration nodes are unreachable from `{entrypoint}`: {unreachable:?}"
        ));
    }
    Ok(())
}

fn verify_openminis_ui_node_route_edges(
    manifest: &serde_json::Map<String, Value>,
    edges: &BTreeSet<OpenMinisUiForwardEdge>,
) -> Result<(), String> {
    let nodes = manifest
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "OpenMinis UI migration manifest missing nodes".to_owned())?;
    for node in nodes {
        let node = node
            .as_object()
            .ok_or_else(|| "OpenMinis UI migration nodes must contain objects".to_owned())?;
        let node_id = required_string(node, "node_id", "OpenMinis UI migration node")?;
        let status = required_string(node, "status", &format!("migration node {node_id}"))?;
        if !matches!(
            status,
            "contract_ready"
                | "implementation_in_progress"
                | "source_bound"
                | "online_verified"
                | "legacy_retired"
                | "blocked_verification_missing"
        ) {
            continue;
        }
        let declared = string_array(
            node.get("route_edge_ids"),
            &format!("migration node {node_id} route_edge_ids"),
        )?;
        let incident = edges
            .iter()
            .filter(|(_, from, to, _)| from == node_id || to == node_id)
            .map(|(edge_id, _, _, _)| edge_id.clone())
            .collect::<BTreeSet<_>>();
        if incident.is_empty() || declared != incident {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` route_edge_ids drift: declared={declared:?}, incident={incident:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn parse_openminis_ui_registered_paths(
    markdown: &str,
) -> Result<
    (
        String,
        BTreeSet<OpenMinisUiForwardEdge>,
        BTreeSet<OpenMinisUiReturnPath>,
    ),
    String,
> {
    let mut in_section = false;
    let mut table = "";
    let mut entrypoint = None;
    let mut edges = BTreeSet::new();
    let mut returns = BTreeSet::new();
    for raw_line in markdown.lines() {
        let line = raw_line.trim();
        if line == "## Entrypoint And Registered Paths" {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if !in_section {
            continue;
        }
        if let Some(value) = line.strip_prefix("- entrypoint_node_id: ") {
            entrypoint = Some(value.trim_matches('`').to_owned());
            continue;
        }
        match line {
            "### Forward Edges" => {
                table = "edges";
                continue;
            }
            "### Return Paths" => {
                table = "returns";
                continue;
            }
            _ => {}
        }
        if line.starts_with("### ") {
            table = "unknown";
            continue;
        }
        if !line.starts_with('|') {
            continue;
        }
        let cells = line
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().trim_matches('`').to_owned())
            .collect::<Vec<_>>();
        let separator = cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|ch| matches!(ch, '-' | ':')));
        match table {
            "edges"
                if cells == ["edge_id", "from_node_id", "to_node_id", "semantic"]
                    || (cells.len() == 4 && separator) => {}
            "returns"
                if cells == ["from_node_id", "to_node_id", "semantic"]
                    || (cells.len() == 3 && separator) => {}
            "edges" if cells.len() == 4 && cells.iter().all(|cell| !cell.is_empty()) => {
                let row = (
                    cells[0].clone(),
                    cells[1].clone(),
                    cells[2].clone(),
                    cells[3].clone(),
                );
                if !edges.insert(row.clone()) {
                    return Err(format!(
                        "OpenMinis UI migration human forward-edge registry has duplicate row {row:?}"
                    ));
                }
            }
            "returns" if cells.len() == 3 && cells.iter().all(|cell| !cell.is_empty()) => {
                let row = (cells[0].clone(), cells[1].clone(), cells[2].clone());
                if !returns.insert(row.clone()) {
                    return Err(format!(
                        "OpenMinis UI migration human return-path registry has duplicate row {row:?}"
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "OpenMinis UI migration human registry has malformed `{table}` table row `{line}`"
                ));
            }
        }
    }
    let entrypoint = entrypoint.ok_or_else(|| {
        "OpenMinis UI migration human tree missing entrypoint_node_id registry".to_owned()
    })?;
    if edges.is_empty() || returns.is_empty() {
        return Err(
            "OpenMinis UI migration human tree must register forward edges and return paths"
                .to_owned(),
        );
    }
    Ok((entrypoint, edges, returns))
}

pub(super) fn parse_openminis_ui_mermaid_tree(
    markdown: &str,
) -> Result<OpenMinisUiMermaidTopology, String> {
    let mut in_mermaid = false;
    let mut aliases = BTreeMap::new();
    let mut arrow_aliases = Vec::new();
    for line in markdown.lines() {
        let line = line.trim();
        if line == "```mermaid" {
            in_mermaid = true;
            continue;
        }
        if in_mermaid && line == "```" {
            break;
        }
        if !in_mermaid || line.is_empty() || line.starts_with("flowchart ") {
            continue;
        }
        if let Some((from, to)) = line.split_once("-->") {
            arrow_aliases.push((from.trim().to_owned(), to.trim().to_owned()));
            continue;
        }
        let Some(open) = line.find('[') else {
            continue;
        };
        let Some(close) = line.rfind(']') else {
            return Err(format!("invalid OpenMinis UI Mermaid declaration `{line}`"));
        };
        let alias = line[..open].trim();
        let label = line[open + 1..close]
            .split(" / ")
            .next()
            .unwrap_or_default()
            .trim();
        if alias.is_empty() || label.is_empty() {
            return Err(format!("invalid OpenMinis UI Mermaid declaration `{line}`"));
        }
        aliases.insert(alias.to_owned(), label.to_owned());
    }
    if !in_mermaid {
        return Err("OpenMinis UI migration doc has no Mermaid tree".to_owned());
    }
    let nodes = aliases.values().cloned().collect::<BTreeSet<_>>();
    let mut edges = BTreeSet::new();
    for (from_alias, to_alias) in arrow_aliases {
        let from = aliases
            .get(&from_alias)
            .ok_or_else(|| format!("unknown Mermaid alias `{from_alias}`"))?;
        let to = aliases
            .get(&to_alias)
            .ok_or_else(|| format!("unknown Mermaid alias `{to_alias}`"))?;
        edges.insert((from.to_owned(), to.to_owned()));
    }
    Ok((nodes, edges))
}
