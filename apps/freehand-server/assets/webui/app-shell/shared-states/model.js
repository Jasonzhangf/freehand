export const SharedUiStateKind = Object.freeze({
  Loading: "loading",
  Empty: "empty",
  Error: "error",
  Confirmation: "confirmation",
});

export function createSharedStateModel(kind, fields) {
  if (!Object.values(SharedUiStateKind).includes(kind)) {
    throw new Error(`unsupported shared UI state: ${kind}`);
  }
  if (!fields || typeof fields !== "object") {
    throw new Error(`shared UI state ${kind} requires fields`);
  }
  const title = requiredText(fields.title, `${kind}.title`);
  const detail = optionalText(fields.detail, `${kind}.detail`);
  const actionLabel = optionalText(fields.actionLabel, `${kind}.actionLabel`);
  const actionId = optionalText(fields.actionId, `${kind}.actionId`);
  if ((actionLabel === "") !== (actionId === "")) {
    throw new Error(`shared UI state ${kind} actionLabel and actionId must be paired`);
  }
  if (kind === SharedUiStateKind.Confirmation && actionId === "") {
    throw new Error("confirmation shared UI state requires an action");
  }
  return Object.freeze({
    kind,
    title,
    detail,
    actionLabel,
    actionId,
  });
}

function requiredText(value, field) {
  const text = optionalText(value, field);
  if (text === "") throw new Error(`shared UI state requires ${field}`);
  return text;
}

function optionalText(value, field) {
  if (value === undefined) return "";
  if (typeof value !== "string") throw new Error(`shared UI state ${field} must be text`);
  return value.trim();
}
