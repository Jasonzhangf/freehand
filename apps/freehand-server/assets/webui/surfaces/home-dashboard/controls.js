export function createHomeSessionRow({ session, markerClass = '', primary, meta, status, turnId = '', live = false, child = false }, context) {
  const item = document.createElement('div');
  item.className = ['mobile-home-session-item', child ? 'is-worker' : '', live ? 'is-live' : ''].filter(Boolean).join(' ');
  const sessionId = (session && session.session_id) || '';
  item.dataset.sessionId = sessionId;
  item.dataset.sessionKind = child ? 'worker' : context.sessionKindLabel(session);
  if (turnId) item.dataset.turnId = turnId;
  if (session && session.task_id) item.dataset.taskId = session.task_id;

  const openButton = document.createElement('button');
  openButton.className = 'mobile-home-session-open';
  openButton.type = 'button';
  openButton.setAttribute('aria-label', `打开会话 ${primary || sessionId || ''}`.trim());

  const marker = document.createElement('span');
  marker.className = ['settings-status-marker', markerClass].filter(Boolean).join(' ');
  marker.setAttribute('aria-hidden', 'true');

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
  openButton.append(marker, copy, statusNode);
  openButton.addEventListener('click', () => context.openSession(sessionId));
  item.appendChild(openButton);

  if (!child && sessionId && !context.isDraftSessionId(sessionId)) {
    const actions = document.createElement('span');
    actions.className = 'mobile-home-session-actions';
    actions.append(
      homeActionButton('rename', '重命名', '改', () => context.renameSession(sessionId)),
      homeActionButton('remove', '移除', '删', () => context.deleteSession(sessionId), 'danger'),
    );
    item.appendChild(actions);
  }
  return item;
}

function homeActionButton(action, label, text, handler, className = '') {
  const button = document.createElement('button');
  button.type = 'button';
  if (className) button.className = className;
  button.dataset.sessionAction = action;
  button.setAttribute('aria-label', `${label}会话`);
  button.title = label;
  button.textContent = text;
  button.addEventListener('click', (event) => {
    event.stopPropagation();
    handler();
  });
  return button;
}
