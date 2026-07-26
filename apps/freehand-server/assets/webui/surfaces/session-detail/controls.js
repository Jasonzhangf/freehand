export function setSelectedSessionId(context, sessionId) {
  const nextSessionId = sessionId || null;
  if (context.state.selectedSessionId !== nextSessionId) {
    context.state.taskHistory = null;
    context.state.workerControl = null;
  }
  context.state.selectedSessionId = nextSessionId;
  if (context.state.selectedSessionId) {
    window.localStorage.setItem(context.selectedSessionStorageKey, context.state.selectedSessionId);
  } else {
    window.localStorage.removeItem(context.selectedSessionStorageKey);
  }
}

export function clearConversationForSessionSwitch(context, sessionId) {
  context.clearSessionRefreshRetryTimer();
  setSelectedSessionId(context, sessionId);
  context.state.sessionTurns = [];
  context.state.turn = null;
  context.state.publicConversation = [];
  context.state.debug = null;
  context.state.adpFailure = null;
  context.state.sessionRefreshInFlight = sessionId;
  context.state.sessionRefreshError = null;
}

export function switchConversationSession(context, sessionId, options = {}) {
  if (!sessionId) return;
  const requestedSessionId = sessionId;
  context.dispatchEdge(options.edgeId || 'home.open_session', options.payload || { session_id: requestedSessionId });
  context.state.sessionTreeOpen = false;
  clearConversationForSessionSwitch(context, sessionId);
  context.renderAll();
  context.refreshSelectedSession().catch((error) => {
    context.renderSessionRefreshFailure(error, requestedSessionId);
  });
}

export function backHomeFromSession(context) {
  context.dispatchEdge('session.back_home');
  context.state.sessionTurns = [];
  context.state.turn = null;
  context.state.publicConversation = [];
  context.state.debug = null;
  context.setCommandStatus('已返回会话首页。', { stickyMs: 4000 });
  context.renderAll();
}
