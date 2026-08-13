import { renderSettingsDiagnosticsSurface } from './diagnostics.js?v=__WEBUI_ASSET_VERSION__';

export function renderSettingsShellSurface(context) {
  const { state, dom } = context;
  const modelLabel =
    state.configStatus?.default_model ||
    dom.modelSelector?.selectedOptions?.[0]?.textContent ||
    dom.modelSelector?.value ||
    '运行时配置';
  const providerSummary = state.configStatus
    ? `${state.configStatus.provider_id} · ${state.configStatus.provider_protocol}`
    : state.configStatusError
      ? '不可用'
      : '加载中';
  context.setText('settings-model-value', modelLabel);
  context.setText('settings-provider-summary', providerSummary);
  context.setText('settings-provider-id', state.configStatus?.provider_id || '加载中');
  context.setText('settings-provider-type', state.configStatus?.provider_type || '加载中');
  context.setText('settings-provider-protocol', state.configStatus?.provider_protocol || '加载中');
  context.setText('settings-provider-host', state.configStatus?.provider_base_url_host || '加载中');
  context.setText('settings-provider-web-search', context.webSearchStatusLabel(state.configStatus));
  context.setTitle('settings-provider-web-search', [
    state.configStatus?.provider_web_search_reason,
    state.configStatus?.provider_web_search_route_summary,
  ].filter(Boolean).join('\n'));
  context.setText(
    'settings-provider-auth',
    state.configStatus
      ? `${context.settingsAuthTypeLabel(state.configStatus.provider_auth_type)} · ${state.configStatus.provider_auth_source}`
      : '加载中',
  );
  context.setText('settings-restart-required', state.configStatus?.restart_required_on_change ? '修改后需要重启' : '未标记需要重启');
  context.setText('settings-config-error', state.configStatusError || '无');
  context.syncProviderSelectionControls();
  context.syncSettingsProviderForm();
  context.renderSettingsProviderRegistry();
  context.syncModelGroupSelectionControls();
  context.syncSettingsModelGroupForm();
  context.renderSettingsModelGroupRegistry();
  context.renderSystemAgentResourceConfig();
  context.renderAccountConfigSync();
  context.renderAndroidApkUpdateSettings();
  renderSettingsDiagnosticsSurface(context);
  renderSettingsNavigationSurface(context);
  context.showInspectorPanel(state.inspectorPanel);
}

export function renderSettingsNavigationSurface(context) {
  const { state, dom } = context;
  const requestedPage = state.settingsPage || 'root';
  const panels = Array.from(document.querySelectorAll('[data-settings-page]'));
  const activePage = panels.some((panel) => panel.dataset.settingsPage === requestedPage)
    ? requestedPage
    : 'root';
  state.settingsPage = activePage;
  panels.forEach((panel) => {
    const active = panel.dataset.settingsPage === activePage;
    panel.hidden = !active;
    panel.dataset.settingsActive = active ? 'true' : 'false';
  });
  document.querySelectorAll('[data-settings-target]').forEach((control) => {
    control.classList.toggle('is-active', control.dataset.settingsTarget === activePage);
  });
  if (dom.settingsShell) {
    dom.settingsShell.dataset.settingsCurrentPage = activePage;
  }
}
