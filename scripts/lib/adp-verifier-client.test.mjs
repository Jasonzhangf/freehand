import assert from 'node:assert/strict';
import { adpVerifierRequest, requireSessionListPage } from './adp-verifier-client.mjs';

class FakeWebSocket {
  constructor(responder) {
    this.listeners = new Map();
    this.sent = [];
    this.responder = responder;
  }

  addEventListener(name, listener) {
    this.listeners.set(name, listener);
  }

  send(value) {
    const frame = JSON.parse(value);
    this.sent.push(frame);
    const response = this.responder(frame);
    if (response) this.receive(response);
  }

  close() {}

  receive(frame) {
    this.listeners.get('message')({ data: JSON.stringify(frame) });
  }

  fail() {
    this.listeners.get('error')(new Error('socket failed'));
  }
}

async function withWebSocket(responder, run) {
  const original = globalThis.WebSocket;
  const socketReady = Promise.withResolvers();
  globalThis.WebSocket = class extends FakeWebSocket {
    constructor(url) {
      super(responder);
      queueMicrotask(() => {
        socketReady.resolve(this);
        this.listeners.get('open')();
      });
    }
  };
  try {
    return { result: await run(), socket: await socketReady.promise };
  } finally {
    globalThis.WebSocket = original;
  }
}

const request = () => adpVerifierRequest({
  url: 'ws://127.0.0.1/adp',
  kind: 'query',
  payloadKey: 'query',
  payload: { Probe: {} },
  clientName: 'shared-client-test',
});

const accepted = await withWebSocket(
  (frame) => frame.kind === 'handshake'
    ? { kind: 'handshake_accepted', request_id: frame.request_id }
    : { kind: 'query_result', request_id: frame.request_id, result: { ok: true } },
  request,
);
assert.equal(accepted.result.ok, true);
assert.equal(accepted.socket.sent[0].protocol_version, 4);
assert.deepEqual(accepted.socket.sent[1].query, { Probe: {} });

await assert.rejects(
  async () => {
    await withWebSocket(
      () => ({ kind: 'failure', failure: { code: 'bad_handshake' } }),
      request,
    );
  },
  /ADP handshake failed/,
);

const validPage = requireSessionListPage({
  SessionListPage: {
    sessions: [],
    page: {
      has_older: false,
      next_cursor: null,
      unavailable_sessions: [],
    },
  },
});
assert.deepEqual(validPage.sessions, []);

const validTerminalPageWithoutCursor = requireSessionListPage({
  SessionListPage: {
    sessions: [],
    page: { has_older: false, unavailable_sessions: [] },
  },
});
assert.equal(validTerminalPageWithoutCursor.page.has_older, false);

const validOlderPage = requireSessionListPage({
  SessionListPage: {
    sessions: [],
    page: { has_older: true, next_cursor: 'older-page', unavailable_sessions: [] },
  },
});
assert.equal(validOlderPage.page.next_cursor, 'older-page');

for (const page of [
  null,
  {},
  {
    sessions: 'not-array',
    page: { has_older: false, next_cursor: null, unavailable_sessions: [] },
  },
  { sessions: [], page: {} },
  { sessions: [], page: { has_older: false, next_cursor: null } },
  { sessions: [], page: { has_older: false, next_cursor: 123, unavailable_sessions: [] } },
  { sessions: [], page: { has_older: true, unavailable_sessions: [] } },
  { sessions: [], page: { has_older: true, next_cursor: null, unavailable_sessions: [] } },
]) {
  assert.throws(() => requireSessionListPage({ SessionListPage: page }), /malformed SessionListPage/);
}
