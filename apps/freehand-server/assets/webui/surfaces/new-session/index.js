import {
  chooseNewTaskDirectory,
  closeNewSessionSurface,
  openNewSessionSurface,
  selectedNewSessionKind,
  submitNewSessionSurface,
  syncNewSessionDialogMode,
} from './controls.js?v=__WEBUI_ASSET_VERSION__';

export const surfaceId = 'new_session';

export const surfaceContract = Object.freeze({
  surfaceId,
  domRootId: 'new-session-dialog',
  role: 'owner_command_sheet',
  owns: Object.freeze(['create_session_command', 'create_task_session_command']),
  entryEdges: Object.freeze(['home.open_new']),
  exitEdges: Object.freeze(['new.created']),
  forbiddenResponsibilities: Object.freeze(['random_verifier_spam', 'create_without_owner_receipt']),
});

export function surfaceRoot(documentRef = document) {
  return documentRef.getElementById(surfaceContract.domRootId);
}

export {
  chooseNewTaskDirectory,
  closeNewSessionSurface,
  openNewSessionSurface,
  selectedNewSessionKind,
  submitNewSessionSurface,
  syncNewSessionDialogMode,
};
