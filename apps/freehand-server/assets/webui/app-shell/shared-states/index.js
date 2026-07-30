export { SharedUiStateKind, createSharedStateModel } from "./model.js?v=__WEBUI_ASSET_VERSION__";
export { renderSharedState } from "./view.js?v=__WEBUI_ASSET_VERSION__";

export const sharedStateContract = Object.freeze({
  contractId: "foundation.shared_states",
  modelResponsibility: "projection_to_render_model_only",
  viewResponsibility: "render_only",
  controlResponsibility: "registered_edge_or_generated_command_only",
  states: Object.freeze(["loading", "empty", "error", "confirmation"]),
});
