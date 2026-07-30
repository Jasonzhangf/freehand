import { SharedUiStateKind, createSharedStateModel } from "../../app-shell/shared-states/model.js?v=__WEBUI_ASSET_VERSION__";
import { renderSharedState } from "../../app-shell/shared-states/view.js?v=__WEBUI_ASSET_VERSION__";

export function renderHomeDashboard(model, context) {
  const root = context.dom.mobileHomeDashboard;
  if (!root) return;
  const activeSessions = model.running;
  context.setText('mobile-home-active-title', activeSessions.length > 0 ? `${activeSessions.length} 个运行中会话` : '暂无运行中会话');
  context.setText(
    'mobile-home-active-copy',
    activeSessions.length > 0
      ? '运行、重试、可自动唤醒的会话都在这里；可以同时存在多个活着的 Agent。'
      : '运行、重试或可自动唤醒的会话会显示在这里。',
  );
  if (context.dom.mobileHomeActiveMarker) {
    context.dom.mobileHomeActiveMarker.classList.toggle('ok', activeSessions.length > 0);
    context.dom.mobileHomeActiveMarker.classList.toggle('attention', activeSessions.length === 0);
  }
  renderRunningList(model, context);
  context.setText('mobile-home-session-count', `${model.history.length} 个历史会话 · 今天 / 过去一周 / 所有更早的`);
  renderHistoryList(model, context);
}

export function renderRunningList(model, context) {
  const list = context.dom.mobileHomeActiveList;
  if (!list) return;
  list.replaceChildren();
  delete list.dataset.sharedState;
  if (model.running.length === 0) {
    renderSharedState(list, createSharedStateModel(
      context.state.sessionListLoaded ? SharedUiStateKind.Empty : SharedUiStateKind.Loading,
      { title: context.state.sessionListLoaded ? '暂无运行中会话。' : '等待活动真源' },
    ));
    return;
  }
  model.running.forEach((observation) => {
    const summary = context.sessionSummaryById(observation.sessionId);
    list.appendChild(context.createSessionRow({
      session: summary || { session_id: observation.sessionId, title: observation.title },
      markerClass: observation.tone === 'phase2-failed' ? 'attention' : 'ok',
      primary: observation.title || observation.sessionId,
      meta: context.liveObservationLine(observation),
      status: observation.label || context.statusLabel(observation.status),
      turnId: observation.turnId,
      live: true,
    }));
  });
}

export function renderHistoryList(model, context) {
  const list = context.dom.mobileHomeSessionList;
  if (!list) return;
  list.replaceChildren();
  list.appendChild(renderBulkActions(context));
  model.buckets.forEach((bucket) => {
    const header = document.createElement('div');
    header.className = 'mobile-home-history-bucket';
    header.dataset.bucket = bucket.id;
    const label = document.createElement('span');
    label.textContent = bucket.label;
    const count = document.createElement('small');
    count.textContent = `${bucket.sessions.length} 个`;
    header.append(label, count);
    list.appendChild(header);
    bucket.sessions.forEach((session) => {
      const children = context.workerChildSessionsForParent(session.session_id);
      list.appendChild(context.createSessionRow({
        session: { ...session, child_count: children.length },
        markerClass: context.sessionHasObservableActiveStatus(session) ? 'ok' : '',
        primary: session.title || session.session_id,
        meta: context.mobileHomeSessionMeta({ ...session, child_count: children.length }),
        status: context.statusLabel(session.latest_status || 'session'),
        live: false,
      }));
    });
  });
  if (model.history.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'mobile-home-empty-row';
    renderSharedState(empty, createSharedStateModel(
      context.state.sessionListLoaded ? SharedUiStateKind.Empty : SharedUiStateKind.Loading,
      { title: context.state.sessionListLoaded ? '暂无历史会话。' : '等待会话真源' },
    ));
    list.appendChild(empty);
  }
}

function renderBulkActions(context) {
  const selectedCount = context.selectedSessionCount();
  const selectableCount = context.selectableSessionCount();
  const row = document.createElement('div');
  row.className = 'mobile-home-bulk-actions';
  row.dataset.selectedCount = `${selectedCount}`;
  row.dataset.selectableCount = `${selectableCount}`;
  const label = document.createElement('span');
  label.textContent = selectedCount > 0 ? `已选 ${selectedCount} 个会话` : '可多选会话';
  const actions = document.createElement('span');
  actions.className = 'mobile-home-bulk-buttons';
  const selectAll = document.createElement('button');
  selectAll.type = 'button';
  selectAll.dataset.sessionAction = 'select-all';
  selectAll.textContent = '全选';
  selectAll.disabled = selectableCount === 0 || selectedCount === selectableCount;
  selectAll.addEventListener('click', () => context.selectAllSessions());
  const clear = document.createElement('button');
  clear.type = 'button';
  clear.dataset.sessionAction = 'clear-selection';
  clear.textContent = '清空';
  clear.disabled = selectedCount === 0;
  clear.addEventListener('click', () => context.clearSelection());
  const remove = document.createElement('button');
  remove.type = 'button';
  remove.className = 'danger';
  remove.dataset.sessionAction = 'remove-selected';
  remove.textContent = '批量移除';
  remove.disabled = selectedCount === 0;
  remove.addEventListener('click', () => context.deleteSelectedSessions());
  actions.append(selectAll, clear, remove);
  row.append(label, actions);
  return row;
}
