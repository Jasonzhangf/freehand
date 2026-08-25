export function renderTimerDashboardSurface(context) {
  renderTimerSourceOptions(context);
  if (context.dom.status) {
    context.dom.status.textContent = context.state.timerStatusError
      ? `定时器查询失败：${context.state.timerStatusError}`
      : context.state.timerList
        ? timerDashboardSummary(context)
        : '等待定时器投影';
  }
  renderTimerDashboardList(context);
  renderTimerDashboardHistory(context);
}

function timerDashboardSummary(context) {
  const stats = timerDashboardStats(context);
  if (!context.state.timerList) return '等待定时器真源';
  const next = stats.next ? ` · 下次 ${context.formatUnixTime(stats.next.next_due_at)}` : '';
  return `${stats.activeCount} 活动 · ${stats.terminalCount} 终态${next}`;
}

function timerDashboardStats(context) {
  const timers = (context.state.timerList && context.state.timerList.timers) || [];
  return {
    totalCount: timers.length,
    activeCount: timers.filter((timer) => ['active', 'running'].includes(timer.status)).length,
    terminalCount: timers.filter((timer) => ['completed', 'cancelled'].includes(timer.status)).length,
    next: timers
      .filter((timer) => ['active', 'running'].includes(timer.status))
      .slice()
      .sort((left, right) => Number(left.next_due_at || 0) - Number(right.next_due_at || 0))[0] || null,
  };
}

function renderTimerSourceOptions(context) {
  const input = context.dom.sourceSessionInput;
  if (!input) return;
  const selectedValue = input.value || context.currentSourceSessionId() || '';
  input.replaceChildren();
  const internal = document.createElement('option');
  internal.value = '';
  internal.textContent = '内部唤醒';
  input.appendChild(internal);
  const isInternalRuntimeSessionId = (sessionId) =>
    sessionId.startsWith('worker-task-')
    || sessionId.startsWith('master-lifecycle-')
    || sessionId.startsWith('master-timer-');
  context.state.sessions.forEach((session) => {
    if (!session || !session.session_id || isInternalRuntimeSessionId(session.session_id)) return;
    const option = document.createElement('option');
    option.value = session.session_id;
    option.textContent = context.compactSentence(session.title || session.session_id, 80);
    input.appendChild(option);
  });
  input.value = Array.from(input.options).some((option) => option.value === selectedValue) ? selectedValue : '';
}

function renderTimerDashboardList(context) {
  const list = context.dom.list;
  if (!list) return;
  list.replaceChildren();
  if (context.state.timerStatusError) {
    list.textContent = context.state.timerStatusError;
    return;
  }
  const timers = ((context.state.timerList && context.state.timerList.timers) || [])
    .slice()
    .sort((left, right) => Number(left.next_due_at || 0) - Number(right.next_due_at || 0));
  if (timers.length === 0) {
    list.textContent = context.state.timerList ? '暂无定时计划。' : '等待定时器真源';
    return;
  }
  timers.forEach((timer) => list.appendChild(timerRow(timer, context)));
}

function timerRow(timer, context) {
  const row = document.createElement('section');
  row.className = 'timer-row';
  row.dataset.timerId = timer.timer_id || '';
  const marker = document.createElement('span');
  marker.className = `settings-status-marker ${['active', 'running'].includes(timer.status) ? 'ok' : 'attention'}`;
  marker.setAttribute('aria-hidden', 'true');
  const body = document.createElement('div');
  body.className = 'timer-row-body';
  const title = document.createElement('strong');
  title.textContent = context.compactSentence(timer.reason || timer.timer_id, 96);
  const meta = document.createElement('small');
  const repeat = timer.repeat_summary ? ` · ${timer.repeat_summary}` : '';
  meta.textContent = context.compactSentence(`${context.statusLabel(timer.status)} · 到期 ${context.formatUnixTime(timer.next_due_at)} · ${timer.fired_count}/${timer.max_runs}${repeat}`, 150);
  const prompt = document.createElement('p');
  prompt.textContent = context.compactSentence(timer.prompt || '没有唤醒提示词', 180);
  body.append(title, meta, prompt);
  row.append(marker, body);
  if (['active', 'running'].includes(timer.status)) {
    const cancel = document.createElement('button');
    cancel.className = 'session-bulk-button timer-cancel-button';
    cancel.type = 'button';
    cancel.textContent = '取消';
    cancel.addEventListener('click', () => context.cancelTimer(timer.timer_id));
    row.appendChild(cancel);
  }
  return row;
}

function renderTimerDashboardHistory(context) {
  const history = context.dom.history;
  if (!history) return;
  history.replaceChildren();
  const events = ((context.state.timerList && context.state.timerList.events) || []).slice(-8).reverse();
  if (events.length === 0) {
    history.textContent = context.state.timerList ? '暂无定时账本事件。' : '等待定时账本';
    return;
  }
  events.forEach((event) => {
    const row = document.createElement('div');
    row.className = 'timer-event-row';
    const marker = document.createElement('span');
    marker.className = `settings-status-marker ${event.event_type === 'TimerScheduled' || event.event_type === 'TimerFired' ? 'ok' : 'attention'}`;
    marker.setAttribute('aria-hidden', 'true');
    const copy = document.createElement('span');
    copy.textContent = context.compactSentence(`${event.event_type} · ${context.formatUnixTime(event.occurred_at)} · ${event.summary}`, 180);
    row.append(marker, copy);
    history.appendChild(row);
  });
}
