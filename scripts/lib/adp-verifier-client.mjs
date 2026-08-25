const DEFAULT_TIMEOUT_MS = 20_000;

function closeSocket(socket) {
  try {
    socket.close();
  } catch {
    // Closing an already-failed socket must not mask the protocol error.
  }
}

export async function adpVerifierRequest({
  url,
  kind,
  payloadKey,
  payload,
  timeoutMs = DEFAULT_TIMEOUT_MS,
  authToken = process.env.FREEHAND_ADP_AUTH_TOKEN || '',
  clientName,
  capabilities = ['adp.v4.handshake'],
  resolveRawMessage = false,
}) {
  if (!url) throw new Error('ADP request requires url');
  if (kind !== 'query' && kind !== 'command') {
    throw new Error(`unsupported ADP verifier frame kind: ${kind}`);
  }
  if (!payloadKey || !clientName) {
    throw new Error('ADP request requires payloadKey and clientName');
  }

  const headers = authToken ? { Authorization: `Bearer ${authToken}` } : undefined;
  const socket = new WebSocket(url, headers ? { headers } : undefined);
  const requestId = `${kind}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const handshakeId = `${requestId}-handshake`;

  return await new Promise((resolve, reject) => {
    let settled = false;
    let handshakeAccepted = false;
    const timer = setTimeout(() => {
      settled = true;
      closeSocket(socket);
      reject(new Error(`ADP ${kind} timeout`));
    }, timeoutMs);

    socket.addEventListener('open', () => {
      socket.send(JSON.stringify({
        protocol_version: 4,
        kind: 'handshake',
        request_id: handshakeId,
        client_name: clientName,
        capabilities,
      }));
    });

    socket.addEventListener('message', (event) => {
      let message;
      try {
        message = JSON.parse(event.data);
      } catch (error) {
        settled = true;
        clearTimeout(timer);
        closeSocket(socket);
        reject(new Error(`ADP ${kind} returned invalid JSON: ${error.message}`));
        return;
      }
      if (!handshakeAccepted) {
        if (message.kind !== 'handshake_accepted' || message.request_id !== handshakeId) {
          settled = true;
          clearTimeout(timer);
          closeSocket(socket);
          reject(new Error(`ADP handshake failed: ${JSON.stringify(message)}`));
          return;
        }
        handshakeAccepted = true;
        socket.send(JSON.stringify({
          protocol_version: 4,
          kind,
          request_id: requestId,
          [payloadKey]: payload,
        }));
        return;
      }
      if (message.request_id !== requestId) return;
      settled = true;
      clearTimeout(timer);
      closeSocket(socket);
      if (message.kind === 'failure') {
        reject(new Error(
          message.failure?.message ||
          message.failure?.code ||
          message.error?.message ||
          JSON.stringify(message.failure || message.error || message),
        ));
        return;
      }
      if (message.kind === 'query_result') {
        if (resolveRawMessage) {
          resolve(message);
          return;
        }
        resolve(message.result);
        return;
      }
      if (message.kind === 'command_receipt') {
        resolve(message.receipt);
        return;
      }
      reject(new Error(`unexpected ADP ${kind} response: ${message.kind}`));
    });

    socket.addEventListener('close', () => {
      if (!settled) reject(new Error(`ADP ${kind} closed before response`));
    });

    socket.addEventListener('error', () => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(new Error(`ADP ${kind} socket error`));
    });
  });
}

export function createAdpVerifierClient(options) {
  return (kind, payloadKey, payload, timeoutMs) =>
    adpVerifierRequest({ ...options, kind, payloadKey, payload, timeoutMs });
}

export function requireQueryVariant(result, variant, label = variant) {
  if (
    !result ||
    typeof result !== 'object' ||
    Array.isArray(result) ||
    !Object.prototype.hasOwnProperty.call(result, variant)
  ) {
    throw new Error(`ADP query expected ${label}, got ${JSON.stringify(result)}`);
  }
  return result[variant];
}

export function requireSessionListPage(result, label = 'session list') {
  const page = requireQueryVariant(result, 'SessionListPage', label);
  if (
    !page ||
    typeof page !== 'object' ||
    Array.isArray(page) ||
    !Array.isArray(page.sessions) ||
    !page.page ||
    typeof page.page !== 'object' ||
    typeof page.page.has_older !== 'boolean' ||
    (page.page.next_cursor !== undefined &&
      page.page.next_cursor !== null &&
      typeof page.page.next_cursor !== 'string') ||
    !Array.isArray(page.page.unavailable_sessions)
  ) {
    throw new Error(`${label} returned malformed SessionListPage: ${JSON.stringify(page)}`);
  }
  return page;
}
