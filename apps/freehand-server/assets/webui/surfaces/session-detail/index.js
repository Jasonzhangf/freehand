import {
  backHomeFromSession,
  clearConversationForSessionSwitch,
  setSelectedSessionId,
  switchConversationSession,
} from './controls.js?v=20260726-session-select-rename';

export const surfaceId = 'session_detail';

export const surfaceContract = Object.freeze({
  surfaceId,
  domRootId: 'message-list',
  role: 'primary_session',
  owns: Object.freeze(['selected_transcript', 'composer', 'session_agent_header']),
  entryEdges: Object.freeze(['home.open_session', 'search.open_result', 'new.created', 'session.open_worker_session']),
  exitEdges: Object.freeze(['session.back_home', 'session.open_agent_sheet', 'session.submit']),
  forbiddenResponsibilities: Object.freeze(['render_global_home_lists', 'synthesize_worker_session_id', 'mutate_session_truth']),
});

export function surfaceRoot(documentRef = document) {
  return documentRef.getElementById(surfaceContract.domRootId);
}

export {
  backHomeFromSession,
  clearConversationForSessionSwitch,
  setSelectedSessionId,
  switchConversationSession,
};
