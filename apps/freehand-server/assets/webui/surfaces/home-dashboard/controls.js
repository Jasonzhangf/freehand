export function createHomeSessionRow({ session, markerClass = '', primary, meta, status, turnId = '', live = false, child = false }, context) {
  const item = document.createElement('div');
  const sessionId = (session && session.session_id) || '';
  const selectable = !child && sessionId && !context.isDraftSessionId(sessionId);
  item.className = [
    'mobile-home-session-item',
    child ? 'is-worker' : '',
    live ? 'is-live' : '',
    selectable ? 'is-selectable' : '',
    selectable && context.isSessionSelected(sessionId) ? 'is-selected' : '',
  ].filter(Boolean).join(' ');
  item.dataset.sessionId = sessionId;
  item.dataset.sessionKind = child ? 'worker' : context.sessionKindLabel(session);
  if (turnId) item.dataset.turnId = turnId;
  if (session && session.task_id) item.dataset.taskId = session.task_id;

  if (selectable) {
    const selectorWrap = document.createElement('label');
    selectorWrap.className = 'mobile-home-session-selector';
    selectorWrap.title = '选择会话';
    const selector = document.createElement('input');
    selector.className = 'mobile-home-session-checkbox';
    selector.type = 'checkbox';
    selector.dataset.sessionAction = 'select';
    selector.checked = context.isSessionSelected(sessionId);
    selector.setAttribute('aria-label', `选择会话 ${primary || sessionId || ''}`.trim());
    selector.addEventListener('change', (event) => {
      event.stopPropagation();
      context.toggleSessionSelection(sessionId, selector.checked);
    });
    selector.addEventListener('click', (event) => event.stopPropagation());
    selectorWrap.appendChild(selector);
    item.appendChild(selectorWrap);
  }

  const openButton = document.createElement('button');
  openButton.className = 'mobile-home-session-open';
  openButton.type = 'button';
  openButton.setAttribute('aria-label', `打开会话 ${primary || sessionId || ''}`.trim());

  const copy = document.createElement('span');
  copy.className = 'mobile-home-session-copy';
  const title = document.createElement('strong');
  title.textContent = context.compactSentence(primary || (session && session.session_id) || '会话', 88);
  const metaNode = document.createElement('small');
  metaNode.textContent = context.compactSentence(meta || '等待会话真源', 120);
  copy.append(title, metaNode);

  const statusNode = document.createElement('span');
  statusNode.className = 'mobile-home-session-status';
  statusNode.textContent = status || '状态';
  openButton.append(copy, statusNode);
  openButton.addEventListener('click', () => context.openSession(sessionId));
  item.appendChild(openButton);
  return item;
}
