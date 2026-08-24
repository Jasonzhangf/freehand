import { adpQueryOf } from "../../generated/adp-protocol.js?v=__WEBUI_ASSET_VERSION__";
import { openAnimatedDialog } from "../../app-shell/dialog-motion.js?v=__WEBUI_ASSET_VERSION__";
export async function openToolsRegistrySurface(context) {
  context.dispatchEdge('root.open_tools');
  openAnimatedDialog(context.dom.dialog);
  context.renderToolsDashboard();
  await context.refreshToolsDashboard();
}

export async function refreshToolsRegistrySurface(context) {
  context.dispatchEdge('tools.refresh');
  context.state.toolRegistryInFlight = true;
  context.renderToolsDashboard();
  try {
    const result = await context.adpQuery(adpQueryOf("QueryToolRegistry"));
    context.applyPhase2QueryResult(result);
    context.setCommandStatus('工具注册表投影已刷新。');
  } catch (error) {
    context.state.toolRegistryError = error.message;
    context.setCommandStatus(`工具注册表刷新失败: ${error.message}`, { stickyMs: 9000 });
  } finally {
    context.state.toolRegistryInFlight = false;
    context.renderToolsDashboard();
  }
}
