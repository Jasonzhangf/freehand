function memoryEntries(state) {
  return Array.isArray(state.memory?.entries) ? state.memory.entries : [];
}

function memorySortLabel(sort) {
  return {
    recent: '最近',
    oldest: '最早',
    relevance: '相关度',
  }[sort] || sort || '最近';
}

export function renderMemorySurface(context) {
  const { state, dom } = context;
  const entries = memoryEntries(state);
  if (dom.status) {
    dom.status.textContent = state.memoryError
      ? `记忆查询失败: ${state.memoryError}`
      : state.memory
        ? `已加载 ${entries.length}/${state.memory.total_matching} 条 · ${memorySortLabel(state.memory.sort)}`
        : 'SQLite 记忆库尚未查询。';
  }
  if (dom.submitButton) {
    dom.submitButton.disabled = state.memoryInFlight;
    dom.submitButton.textContent = state.memoryInFlight ? '查询中...' : '查询';
  }
  if (dom.loadMoreButton) {
    dom.loadMoreButton.hidden = !state.memory?.has_older;
    dom.loadMoreButton.disabled = state.memoryInFlight;
  }
  if (!dom.results) return;
  dom.results.replaceChildren();
  if (state.memoryError) {
    dom.results.textContent = state.memoryError;
    return;
  }
  if (state.memoryInFlight && entries.length === 0) {
    dom.results.textContent = '正在查询 SQLite 记忆索引...';
    return;
  }
  if (entries.length === 0) {
    dom.results.textContent = state.memory ? '没有匹配的记忆。' : '尚未查询。';
    return;
  }
  entries.forEach((entry) => {
    dom.results.appendChild(renderMemoryEntry(entry, context));
  });
}

export function renderMemoryEntry(entry, context) {
  const card = document.createElement('article');
  card.className = 'memory-card';
  card.dataset.memoryId = `${entry.id || ''}`;

  const header = document.createElement('div');
  header.className = 'memory-card-head';
  const title = document.createElement('strong');
  title.textContent = entry.tool_call_id || entry.turn_id || entry.session_id || '记忆条目';
  const meta = document.createElement('small');
  meta.textContent = [
    entry.session_id,
    entry.turn_id,
    entry.tool_call_id,
    context.formatMemoryTime?.(entry.created_at_unix_seconds),
  ].filter(Boolean).join(' · ');
  header.append(title, meta);

  const content = document.createElement('pre');
  content.className = 'memory-card-content';
  content.textContent = entry.content || '';

  const actions = document.createElement('div');
  actions.className = 'memory-card-actions';
  const copy = document.createElement('button');
  copy.type = 'button';
  copy.className = 'memory-card-copy';
  copy.setAttribute('aria-label', '复制记忆内容');
  copy.title = '复制记忆内容';
  copy.textContent = '复制';
  copy.addEventListener('click', () => context.copyMemory(entry.content || '', copy));
  actions.appendChild(copy);

  card.append(header, content, actions);
  return card;
}
