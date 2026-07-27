import assert from 'node:assert/strict';

import {
  HISTORICAL_FAILURE_RECOVERED,
  historicalFailureRecoveredLifecycle,
  historicalFailureRecoveredRows,
  historicalRecoveryProjectionChanged,
  recoveredHistoricalWorkerFailureTurnIds,
} from '../apps/freehand-server/assets/webui/surfaces/session-detail/recovery.js';

const failed = { turn_id: 'worker-turn-old', terminal_status: 'Failed' };
const success = { turn_id: 'worker-turn-new', terminal_status: 'Success' };

assert.deepEqual(
  [...recoveredHistoricalWorkerFailureTurnIds([failed, success], {
    isWorkerSession: true,
    taskStatus: 'closed',
  })],
  ['worker-turn-old'],
);
assert.equal(recoveredHistoricalWorkerFailureTurnIds([failed, success], {
  isWorkerSession: true,
  taskStatus: 'running',
}).size, 0);
assert.equal(recoveredHistoricalWorkerFailureTurnIds([success, failed], {
  isWorkerSession: true,
  taskStatus: 'closed',
}).size, 0);
assert.equal(recoveredHistoricalWorkerFailureTurnIds([failed, success], {
  isWorkerSession: false,
  taskStatus: 'closed',
}).size, 0);

const rows = historicalFailureRecoveredRows([
  { kind: 'final', title: '最终结果', body: ['provider 401'], status: 'failed' },
  { kind: 'error', title: 'Error', body: ['invalid api key'], status: 'failed' },
]);
assert.equal(rows.length, 1);
assert.equal(rows[0].kind, 'final');
assert.equal(rows[0].title, '历史失败 · 后续已恢复');
assert.equal(rows[0].status, '后续已恢复');
assert.doesNotMatch(JSON.stringify(rows), /401|invalid api key/i);

const lifecycle = historicalFailureRecoveredLifecycle();
assert.equal(lifecycle.phase, HISTORICAL_FAILURE_RECOVERED);
assert.equal(lifecycle.label, '历史失败 · 后续已恢复');
assert.equal(lifecycle.isLive, false);

assert.equal(historicalRecoveryProjectionChanged(
  { recoveryState: '', recoveryDebugDetails: '' },
  { recoveryState: HISTORICAL_FAILURE_RECOVERED, recoveryDebugDetails: 'false' },
), true);
assert.equal(historicalRecoveryProjectionChanged(
  { recoveryState: HISTORICAL_FAILURE_RECOVERED, recoveryDebugDetails: 'false' },
  { recoveryState: HISTORICAL_FAILURE_RECOVERED, recoveryDebugDetails: 'true' },
), true);
assert.equal(historicalRecoveryProjectionChanged(
  { recoveryState: '', recoveryDebugDetails: 'false' },
  { recoveryState: '', recoveryDebugDetails: 'true' },
), false);

console.log('session_detail_recovery_model_ok positive=closed_later_success negative=latest_failure_or_nonclosed');
