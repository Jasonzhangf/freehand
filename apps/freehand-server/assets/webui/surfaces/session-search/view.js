export function renderSessionSearchSurface(context) {
  const { state, dom } = context;
  if (dom.status) {
    dom.status.textContent = state.sessionSearchError
      ? `会话搜索失败: ${state.sessionSearchError}`
      : state.sessionSearch
        ? `查询「${state.sessionSearch.query || ""}」· ${sessionSearchResultsList(state).length} 个父结果`
        : '输入关键词搜索持久化会话。';
  }
  if (dom.submitButton) {
    dom.submitButton.disabled = state.sessionSearchInFlight;
    dom.submitButton.textContent = state.sessionSearchInFlight ? '搜索中...' : '搜索';
  }
  if (!dom.results) return;
  dom.results.replaceChildren();
  if (state.sessionSearchError) {
    dom.results.textContent = state.sessionSearchError;
    return;
  }
  if (state.sessionSearchInFlight) {
    dom.results.textContent = '正在查询持久化会话索引...';
    return;
  }
  const results = sessionSearchResultsList(state);
  if (results.length === 0) {
    dom.results.textContent = state.sessionSearch ? '没有匹配的持久化会话。' : '尚未查询。';
    return;
  }
  results.forEach((result) => {
    dom.results.appendChild(renderSessionSearchResult(result, context));
  });
}

export function sessionSearchResultsList(state) {
  return Array.isArray(state.sessionSearch?.results) ? state.sessionSearch.results : [];
}

export function renderSessionSearchResult(result, context) {
  const card = document.createElement('article');
  card.className = 'session-search-card';
  card.dataset.sessionId = result.session_id || '';
  const head = document.createElement('button');
  head.className = 'session-search-card-head';
  head.type = 'button';
  const marker = document.createElement('span');
  marker.className = 'settings-status-marker ok';
  marker.setAttribute('aria-hidden', 'true');
  const title = document.createElement('span');
  title.className = 'session-search-title';
  const strong = document.createElement('strong');
  strong.textContent = context.compactSentence(result.title || result.session_id || 'session', 96);
  const small = document.createElement('small');
  small.textContent = context.compactSentence([
    result.latest_status || 'session',
    result.latest_turn_id ? `turn ${result.latest_turn_id}` : '',
    result.cwd || '',
  ].filter(Boolean).join(' · '), 110);
  title.append(strong, small);
  head.append(marker, title);
  head.addEventListener('click', () => context.openResult(result.session_id));
  card.appendChild(head);

  const snippet = document.createElement('p');
  snippet.className = 'session-search-snippet';
  snippet.textContent = result.snippet || '匹配到持久化会话元数据。';
  card.appendChild(snippet);

  const fields = document.createElement('div');
  fields.className = 'session-search-fields';
  fields.textContent = `匹配字段：${(result.matched_fields || []).join(', ') || 'session'}`;
  card.appendChild(fields);

  const childMatches = Array.isArray(result.child_matches) ? result.child_matches : [];
  if (childMatches.length > 0) {
    const children = document.createElement('div');
    children.className = 'session-search-child-list';
    childMatches.forEach((child) => {
      const childRow = document.createElement('button');
      childRow.className = 'session-search-child';
      childRow.type = 'button';
      childRow.dataset.parentSessionId = result.session_id || '';
      childRow.dataset.childSessionId = child.session_id || '';
      childRow.textContent = context.compactSentence([
        `工作器子项: ${child.title || child.task_id || child.session_id}`,
        child.latest_status || 'session',
        child.snippet || '',
      ].filter(Boolean).join(' · '), 180);
      childRow.addEventListener('click', () => context.openResult(result.session_id));
      children.appendChild(childRow);
    });
    card.appendChild(children);
  }
  return card;
}
