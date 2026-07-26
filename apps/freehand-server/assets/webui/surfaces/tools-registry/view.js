export function renderToolsRegistrySurface(context) {
  if (context.dom.status) {
    context.dom.status.textContent = context.state.toolRegistryError
      ? `工具注册表查询失败: ${context.state.toolRegistryError}`
      : context.state.toolRegistry
        ? toolRegistrySummary(context)
        : '等待工具注册表投影';
  }
  if (context.dom.refreshButton) {
    context.dom.refreshButton.disabled = context.state.toolRegistryInFlight;
    context.dom.refreshButton.textContent = context.state.toolRegistryInFlight ? '刷新中...' : '刷新工具';
  }
  renderGuidance(context);
  renderTools(context);
}

function toolRegistrySummary(context) {
  const tools = context.toolRegistryTools();
  const masterCount = tools.filter((tool) => tool.exposed_to_master).length;
  const workerCount = tools.filter((tool) => tool.exposed_to_worker).length;
  const unimplementedCount = tools.filter((tool) => !tool.implemented).length;
  return `registry ${context.state.toolRegistry.registry_version || 'unknown'} · ${tools.length} tools · master ${masterCount} · worker ${workerCount} · unimplemented ${unimplementedCount}`;
}

function renderGuidance(context) {
  const guidanceNode = context.dom.guidance;
  if (!guidanceNode) return;
  guidanceNode.replaceChildren();
  if (context.state.toolRegistryError) {
    guidanceNode.textContent = context.state.toolRegistryError;
    return;
  }
  const guidance = Array.isArray(context.state.toolRegistry?.guidance) ? context.state.toolRegistry.guidance : [];
  if (guidance.length === 0) {
    guidanceNode.textContent = context.state.toolRegistry ? '没有注册表引导。' : '等待注册表引导';
    return;
  }
  guidance.forEach((line) => {
    const item = document.createElement('p');
    item.textContent = line;
    guidanceNode.appendChild(item);
  });
}

function renderTools(context) {
  const list = context.dom.list;
  if (!list) return;
  list.replaceChildren();
  if (context.state.toolRegistryError) {
    list.textContent = context.state.toolRegistryError;
    return;
  }
  const tools = context.toolRegistryTools();
  if (tools.length === 0) {
    list.textContent = context.state.toolRegistry ? '没有工具注册表行。' : '等待工具注册表真源';
    return;
  }
  tools.forEach((tool) => list.appendChild(renderToolCard(tool, context)));
}

function renderToolCard(tool, context) {
  const card = document.createElement('article');
  card.className = 'tool-registry-card';
  card.dataset.toolName = tool.name || '';
  card.dataset.scope = tool.execution_scope || '';
  card.dataset.implemented = String(tool.implemented === true);
  card.dataset.readOnly = String(tool.read_only === true);
  card.dataset.exposedToMaster = String(tool.exposed_to_master === true);
  card.dataset.exposedToWorker = String(tool.exposed_to_worker === true);

  const header = document.createElement('div');
  header.className = 'tool-registry-card-head';
  const marker = document.createElement('span');
  marker.className = `settings-status-marker ${toolRegistryTone(tool)}`;
  marker.setAttribute('aria-hidden', 'true');
  const title = document.createElement('div');
  title.className = 'tool-registry-title';
  const name = document.createElement('strong');
  name.textContent = tool.name || 'unnamed';
  const meta = document.createElement('small');
  meta.textContent = [
    `scope=${tool.execution_scope || 'unknown'}`,
    `read_only=${tool.read_only === true}`,
    `implemented=${tool.implemented === true}`,
    `master=${tool.exposed_to_master === true}`,
    `worker=${tool.exposed_to_worker === true}`,
  ].join(' · ');
  title.append(name, meta);
  header.append(marker, title);
  card.appendChild(header);

  const description = document.createElement('p');
  description.className = 'tool-registry-description';
  description.textContent = tool.description || '没有投影说明。';
  card.appendChild(description);

  const badges = document.createElement('div');
  badges.className = 'tool-registry-badges';
  badges.append(
    badge(tool.execution_scope || 'unknown', 'scope'),
    badge(tool.read_only ? '只读' : '会修改', 'read_only'),
    badge(tool.implemented ? 'implemented' : 'unimplemented', 'implemented'),
    badge(tool.exposed_to_master ? '主控可见' : '主控隐藏', 'master'),
    badge(tool.exposed_to_worker ? '工作器可见' : '工作器隐藏', 'worker'),
  );
  card.appendChild(badges);

  appendListSection(card, 'Examples', tool.examples, 'example');
  appendListSection(card, 'Guidance', tool.guidance, 'guidance');
  const details = document.createElement('details');
  details.className = 'tool-registry-schema';
  const summary = document.createElement('summary');
  summary.textContent = '输入结构';
  const pre = document.createElement('pre');
  pre.textContent = schemaPreview(tool.input_schema);
  details.append(summary, pre);
  card.appendChild(details);
  return card;
}

function toolRegistryTone(tool) {
  if (!tool || tool.implemented !== true) return 'attention';
  return tool.exposed_to_master || tool.exposed_to_worker ? 'ok' : 'attention';
}

function badge(label, key) {
  const badgeNode = document.createElement('span');
  badgeNode.className = 'tool-registry-badge';
  badgeNode.dataset.badge = key || '';
  badgeNode.textContent = label;
  return badgeNode;
}

function appendListSection(card, title, items, kind) {
  const values = Array.isArray(items) ? items.filter(Boolean) : [];
  if (values.length === 0) return;
  const section = document.createElement('section');
  section.className = `tool-registry-section tool-registry-section-${kind}`;
  const heading = document.createElement('div');
  heading.className = 'tool-registry-section-heading';
  heading.textContent = title;
  section.appendChild(heading);
  values.forEach((value) => {
    const item = document.createElement(kind === 'example' ? 'code' : 'p');
    item.textContent = `${value}`;
    section.appendChild(item);
  });
  card.appendChild(section);
}

function schemaPreview(schema) {
  try {
    return JSON.stringify(schema || {}, null, 2);
  } catch (_) {
    return String(schema || '');
  }
}
