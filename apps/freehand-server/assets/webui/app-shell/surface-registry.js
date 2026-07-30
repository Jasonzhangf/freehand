import { surfaceContract as homeDashboardSurface } from "../surfaces/home-dashboard/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as sessionDetailSurface } from "../surfaces/session-detail/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as toolsRegistrySurface } from "../surfaces/tools-registry/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as timerDashboardSurface } from "../surfaces/timer-dashboard/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as settingsSurface } from "../surfaces/settings/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as sessionSearchSurface } from "../surfaces/session-search/index.js?v=__WEBUI_ASSET_VERSION__";
import { surfaceContract as newSessionSurface } from "../surfaces/new-session/index.js?v=__WEBUI_ASSET_VERSION__";

const surfaceContractRegistry = Object.freeze({
  home_dashboard: homeDashboardSurface,
  session_detail: sessionDetailSurface,
  tools_registry: toolsRegistrySurface,
  timer_dashboard: timerDashboardSurface,
  settings: settingsSurface,
  session_search: sessionSearchSurface,
  new_session: newSessionSurface,
});

const arrayFields = Object.freeze([
  "owns",
  "entryEdges",
  "exitEdges",
  "forbiddenResponsibilities",
]);

export function validateSurfaceContractRegistry(registry) {
  if (!registry || typeof registry !== "object" || Array.isArray(registry)) {
    throw new Error("surface contract registry must be an object");
  }
  const entries = Object.entries(registry);
  if (entries.length === 0) {
    throw new Error("surface contract registry must not be empty");
  }
  const surfaceIds = new Set();
  const domRootIds = new Set();
  for (const [registeredId, contract] of entries) {
    if (!contract || typeof contract !== "object" || !Object.isFrozen(contract)) {
      throw new Error(`surface contract ${registeredId} must be an immutable object`);
    }
    for (const field of ["surfaceId", "domRootId", "role"]) {
      if (typeof contract[field] !== "string" || contract[field].trim() === "") {
        throw new Error(`surface contract ${registeredId}.${field} must be a non-empty string`);
      }
    }
    if (contract.surfaceId !== registeredId) {
      throw new Error(`surface contract registry key ${registeredId} does not match ${contract.surfaceId}`);
    }
    if (surfaceIds.has(contract.surfaceId)) {
      throw new Error(`duplicate surface id ${contract.surfaceId}`);
    }
    if (domRootIds.has(contract.domRootId)) {
      throw new Error(`duplicate surface DOM root ${contract.domRootId}`);
    }
    surfaceIds.add(contract.surfaceId);
    domRootIds.add(contract.domRootId);
    for (const field of arrayFields) {
      if (!Array.isArray(contract[field]) || !Object.isFrozen(contract[field]) || contract[field].length === 0) {
        throw new Error(`surface contract ${registeredId}.${field} must be a non-empty immutable array`);
      }
      if (contract[field].some((value) => typeof value !== "string" || value.trim() === "")) {
        throw new Error(`surface contract ${registeredId}.${field} must contain non-empty strings`);
      }
    }
  }
  return Object.freeze(entries.map(([, contract]) => contract));
}

export const surfaceContracts = validateSurfaceContractRegistry(surfaceContractRegistry);
