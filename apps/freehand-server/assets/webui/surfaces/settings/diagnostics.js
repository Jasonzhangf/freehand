export function renderSettingsDiagnosticsSurface(context) {
  const { state, dom } = context;
  const files = Array.isArray(state.diagnostics?.files) ? state.diagnostics.files : [];
  context.setText(
    'settings-diagnostics-summary',
    state.diagnosticsError
      ? '查询失败'
      : state.diagnostics
        ? `${files.length} 个日志文件`
        : '加载中',
  );
  context.setText('settings-diagnostics-runtime-home', state.diagnostics?.runtime_home || '加载中');
  if (dom.diagnosticsStatus) {
    dom.diagnosticsStatus.textContent = state.diagnosticsError
      ? `诊断查询失败：${state.diagnosticsError}`
      : state.diagnostics
        ? `来自 ${state.diagnostics.logs_dir || 'logs'} 的脱敏日志元数据 · 生成 ${context.formatUnixTime(state.diagnostics.generated_at)}`
        : '诊断只显示服务拥有的日志元数据和脱敏尾部内容。';
  }
  if (dom.diagnosticsRefreshButton) {
    dom.diagnosticsRefreshButton.disabled = state.diagnosticsInFlight;
    dom.diagnosticsRefreshButton.textContent = state.diagnosticsInFlight
      ? '正在刷新诊断...'
      : '刷新诊断';
  }
  if (!dom.diagnosticsList) return;
  dom.diagnosticsList.replaceChildren();
  if (state.diagnosticsError) {
    dom.diagnosticsList.textContent = state.diagnosticsError;
    return;
  }
  if (files.length === 0) {
    dom.diagnosticsList.textContent = state.diagnostics ? '没有投影日志文件。' : '等待诊断投影';
    return;
  }
  files.slice(0, 8).forEach((file) => {
    dom.diagnosticsList.appendChild(renderDiagnosticLogRow(file, context));
  });
}

export function renderDiagnosticLogRow(file, context) {
  const row = document.createElement('article');
  row.className = 'settings-diagnostic-row diagnostic-log-row';
  row.dataset.logName = file.name || '';
  row.dataset.relativePath = file.relative_path || '';
  const marker = document.createElement('span');
  marker.className = 'settings-status-marker ok';
  marker.setAttribute('aria-hidden', 'true');
  const copy = document.createElement('span');
  const label = document.createElement('strong');
  label.textContent = file.name || file.relative_path || '日志文件';
  const meta = document.createElement('small');
  meta.textContent = context.compactSentence(
    `${file.relative_path || 'logs'} · ${Number(file.size_bytes || 0)} bytes · ${context.formatUnixTime(file.modified_at)}`,
    150,
  );
  const tail = document.createElement('small');
  tail.textContent = context.compactSentence((file.tail_lines || []).join(' / ') || '没有尾部日志', 180);
  copy.append(label, meta, tail);
  row.append(marker, copy);
  return row;
}
