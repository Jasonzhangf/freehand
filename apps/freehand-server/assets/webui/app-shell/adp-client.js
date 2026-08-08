import {
  ADP_HANDSHAKE_CAPABILITY,
  ADP_PROTOCOL_VERSION,
} from "../generated/adp-protocol.js?v=__WEBUI_ASSET_VERSION__";

function versionedAdpFrame(frame) {
  return { protocol_version: ADP_PROTOCOL_VERSION, ...frame };
}

export function settleAdpResponseFrame({ state, windowRef, frame }) {
  const request = state.adpRequests.get(frame.request_id);
  const settleRequest = (settle) => {
    if (!request) {
      return false;
    }
    state.adpRequests.delete(frame.request_id);
    windowRef.clearTimeout(request.timeoutId);
    settle(request);
    return true;
  };

  switch (frame.kind) {
    case 'query_result':
      return { kind: frame.kind, settled: settleRequest((pending) => pending.resolve(frame.result)) };
    case 'command_receipt':
      return { kind: frame.kind, settled: settleRequest((pending) => pending.resolve(frame.receipt)), receipt: frame.receipt };
    case 'subscription_accepted':
      if (request) {
        state.adpSubscriptions.add(frame.request_id);
      }
      return {
        kind: frame.kind,
        settled: settleRequest((pending) => pending.resolve(frame.selector)),
        selector: frame.selector,
      };
    case 'subscription_event':
      return { kind: frame.kind, settled: false, event: frame.event };
    case 'failure': {
      const failure = frame.failure;
      if (
        !failure
        || typeof failure.code !== 'string'
        || failure.code.length === 0
        || typeof failure.message !== 'string'
        || failure.message.length === 0
      ) {
        throw new Error('ADP failure frame violates the generated protocol contract');
      }
      return {
        kind: frame.kind,
        settled: settleRequest((pending) => pending.reject(new Error(failure.message))),
        failure,
      };
    }
    default:
      return { kind: frame.kind, settled: false, unsupported: true };
  }
}

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
    if (state.adpOpened) {
      return state.adpOpened;
    }
    if (state.adpSocket && state.adpSocket.readyState === WebSocketCtor.OPEN) {
      return Promise.resolve(state.adpSocket);
    }

    const socket = new WebSocketCtor(url());
    state.adpSocket = socket;
    state.adpStatus = 'connecting';
    setCommandStatus('正在连接服务...');

    state.adpOpened = new Promise((resolve, reject) => {
      socket.addEventListener('open', () => {
        const handshakeId = nextRequestId('hello');
        state.adpHandshakeRequestId = handshakeId;
        try {
          socket.send(JSON.stringify(versionedAdpFrame({
            kind: 'handshake',
            request_id: handshakeId,
            client_name: 'freehand-webui',
            capabilities: [ADP_HANDSHAKE_CAPABILITY],
          })));
        } catch (error) {
          state.adpStatus = 'error';
          state.adpOpened = null;
          state.adpHandshakeRequestId = null;
          state.adpFailure = `连接握手失败：${error.message}`;
          setCommandStatus(state.adpFailure);
          renderAll();
          reject(error);
          try {
            socket.close();
          } catch (_) {}
        }
      });
      socket.addEventListener('message', (event) => {
        try {
          const frame = JSON.parse(event.data);
          if (frame.protocol_version !== ADP_PROTOCOL_VERSION) {
            throw new Error(`服务协议版本不匹配：${frame.protocol_version ?? '缺失'}`);
          }
          if (state.adpHandshakeRequestId) {
            if (frame.kind === 'handshake_accepted' && frame.request_id === state.adpHandshakeRequestId) {
              state.adpHandshakeRequestId = null;
              state.adpStatus = 'connected';
              state.adpFailure = null;
              state.adpReconnectAttempt = 0;
              clearReconnectTimer();
              setCommandStatus('已连接，等待更新...');
              renderAll();
              resolve(socket);
              return;
            }
            if (frame.kind === 'failure' && frame.request_id === state.adpHandshakeRequestId) {
              state.adpHandshakeRequestId = null;
              state.adpOpened = null;
              state.adpStatus = 'error';
              state.adpFailure = '连接握手失败：服务拒绝连接';
              setCommandStatus(state.adpFailure);
              renderAll();
              reject(new Error(state.adpFailure));
              try {
                socket.close();
              } catch (_) {}
              return;
            }
            throw new Error(`连接握手失败：收到 ${frame.kind || '未知'} 响应`);
          }
          handleFrame(frame);
        } catch (error) {
          state.adpFailure = `连接解码失败：${error.message}`;
          if (state.adpHandshakeRequestId) {
            state.adpStatus = 'error';
            state.adpOpened = null;
            state.adpHandshakeRequestId = null;
            reject(error);
            try {
              socket.close();
            } catch (_) {}
          }
          setCommandStatus(state.adpFailure);
          renderAll();
        }
      });
      socket.addEventListener('error', () => {
        state.adpStatus = 'error';
        state.adpOpened = null;
        state.adpHandshakeRequestId = null;
        setCommandStatus('连接错误');
        renderAll();
        reject(new Error('连接错误'));
      });
      socket.addEventListener('close', () => {
        state.adpStatus = 'closed';
        setCommandStatus('连接已关闭');
        state.adpSocket = null;
        state.adpOpened = null;
        state.adpHandshakeRequestId = null;
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
    socket.send(JSON.stringify(versionedAdpFrame(frame)));
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
