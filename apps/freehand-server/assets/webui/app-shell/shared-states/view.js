export function renderSharedState(container, model, options) {
  if (!container) {
    throw new Error("shared UI state requires a container");
  }
  container.replaceChildren();
  container.dataset.sharedState = model.kind;

  const documentRef = container.ownerDocument;
  const status = documentRef.createElement("section");
  status.className = `shared-ui-state shared-ui-state-${model.kind}`;
  status.setAttribute("role", model.kind === "error" ? "alert" : "status");

  if (model.title) {
    const title = documentRef.createElement("strong");
    title.className = "shared-ui-state-title";
    title.textContent = model.title;
    status.append(title);
  }
  if (model.detail) {
    const detail = documentRef.createElement("p");
    detail.className = "shared-ui-state-detail";
    detail.textContent = model.detail;
    status.append(detail);
  }
  if (model.actionLabel && model.actionId) {
    if (!options || typeof options.onAction !== "function") {
      throw new Error("actionable shared UI state requires onAction");
    }
    const action = documentRef.createElement("button");
    action.type = "button";
    action.className = "shared-ui-state-action";
    action.dataset.actionId = model.actionId;
    action.textContent = model.actionLabel;
    action.addEventListener("click", () => options.onAction(model.actionId));
    status.append(action);
  }
  container.append(status);
}
