export function createAdpClient({
  state,
  windowRef,
  WebSocketCtor,
  url,
  nextRequestId,
  formatDuration,
  setCommandStatus,
  renderAll,
  scheduleReconnect,
  clearReconnectTimer,
  handleFrame,
  requestTimeoutMs,
}) {
  function ensureSocket() {
    if (state.adpSocket && state.adpSocket.readyState === WebSocketCtor.OPEN) {
      return Promise.resolve(state.adpSocket);
    }
    if (state.adpOpened) {
      return state.adpOpened;
    }

    const socket = new WebSocketCtor(url());
    state.adpSocket = socket;
    state.adpStatus = 'connecting';
    setCommandStatus('正在连接服务...');

    state.adpOpened = new Promise((resolve, reject) => {
      socket.addEventListener('open', () => {
        state.adpStatus = 'connected';
        state.adpFailure = null;
        state.adpReconnectAttempt = 0;
        clearReconnectTimer();
        setCommandStatus('已连接，等待更新...');
        renderAll();
        resolve(socket);
      });
      socket.addEventListener('message', (event) => {
        try {
          handleFrame(JSON.parse(event.data));
        } catch (error) {
          state.adpFailure = `连接解码失败：${error.message}`;
          setCommandStatus(state.adpFailure);
          renderAll();
        }
      });
      socket.addEventListener('error', () => {
        state.adpStatus = 'error';
        setCommandStatus('连接错误');
        renderAll();
        reject(new Error('连接错误'));
      });
      socket.addEventListener('close', () => {
        state.adpStatus = 'closed';
        setCommandStatus('连接已关闭');
        state.adpSocket = null;
        state.adpOpened = null;
        state.adpSubscriptions.clear();
        for (const { reject: rejectRequest } of state.adpRequests.values()) {
          rejectRequest(new Error('连接已关闭'));
        }
        for (const { timeoutId } of state.adpRequests.values()) {
          windowRef.clearTimeout(timeoutId);
        }
        state.adpRequests.clear();
        renderAll();
        scheduleReconnect('传输关闭');
      });
    });

    return state.adpOpened;
  }

  async function send(frame) {
    const socket = await ensureSocket();
    socket.send(JSON.stringify(frame));
  }

  function request(kind, payloadKey, payload, prefix) {
    const requestId = nextRequestId(prefix);
    const frame = { kind, request_id: requestId };
    frame[payloadKey] = payload;
    const promise = new Promise((resolve, reject) => {
      const timeoutId = windowRef.setTimeout(() => {
        if (!state.adpRequests.has(requestId)) return;
        state.adpRequests.delete(requestId);
        reject(new Error(`request timed out after ${formatDuration(requestTimeoutMs)}`));
      }, requestTimeoutMs);
      state.adpRequests.set(requestId, { resolve, reject, kind, timeoutId });
    });
    send(frame).catch((error) => {
      const request = state.adpRequests.get(requestId);
      state.adpRequests.delete(requestId);
      if (request) {
        windowRef.clearTimeout(request.timeoutId);
        request.reject(error);
      }
    });
    return promise;
  }

  return Object.freeze({
    ensureSocket,
    send,
    request,
    query(query) {
      return request('query', 'query', query, 'query');
    },
    command(command) {
      return request('command', 'command', command, 'cmd');
    },
    subscribe(subscription, prefix) {
      return request('subscribe', 'subscription', subscription, prefix);
    },
  });
}
