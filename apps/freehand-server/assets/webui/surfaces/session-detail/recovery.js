export const HISTORICAL_FAILURE_RECOVERED = 'historical_failure_recovered';

function terminalStatus(turn) {
  return `${turn && turn.terminal_status ? turn.terminal_status : ''}`.trim().toLowerCase();
}

export function recoveredHistoricalWorkerFailureTurnIds(turns, options = {}) {
  const orderedTurns = Array.isArray(turns) ? turns.filter(Boolean) : [];
  const isWorkerSession = options.isWorkerSession === true;
  const taskStatus = `${options.taskStatus || ''}`.trim().toLowerCase();
  if (!isWorkerSession || taskStatus !== 'closed') {
    return new Set();
  }

  const recovered = new Set();
  let laterSuccess = false;
  for (let index = orderedTurns.length - 1; index >= 0; index -= 1) {
    const turn = orderedTurns[index];
    const status = terminalStatus(turn);
    if (status === 'success') {
      laterSuccess = true;
      continue;
    }
    if (laterSuccess && status === 'failed' && turn.turn_id) {
      recovered.add(turn.turn_id);
    }
  }
  return recovered;
}

export function historicalFailureRecoveredLifecycle() {
  return {
    phase: HISTORICAL_FAILURE_RECOVERED,
    className: 'pending',
    label: '历史失败 · 后续已恢复',
    isLive: false,
    neutral: false,
    elapsed: '',
  };
}

export function historicalFailureRecoveredRows(rows) {
  const sourceRows = Array.isArray(rows) ? rows : [];
  const visibleRows = sourceRows
    .filter((row) => row && row.kind !== 'error')
    .map((row) => row.kind === 'final'
      ? {
          ...row,
          title: '历史失败 · 后续已恢复',
          body: ['本次 Worker 执行失败；同一任务已由后续执行恢复并关闭。'],
          status: '后续已恢复',
        }
      : row);
  if (visibleRows.some((row) => row.kind === 'final')) {
    return visibleRows;
  }
  visibleRows.push({
    kind: 'final',
    title: '历史失败 · 后续已恢复',
    body: ['本次 Worker 执行失败；同一任务已由后续执行恢复并关闭。'],
    status: '后续已恢复',
    identity: null,
  });
  return visibleRows;
}

export function historicalRecoveryProjectionChanged(existingProjection, nextProjection) {
  const existingState = `${existingProjection && existingProjection.recoveryState ? existingProjection.recoveryState : ''}`;
  const nextState = `${nextProjection && nextProjection.recoveryState ? nextProjection.recoveryState : ''}`;
  if (existingState !== HISTORICAL_FAILURE_RECOVERED && nextState !== HISTORICAL_FAILURE_RECOVERED) {
    return false;
  }
  const existingDebug = `${existingProjection && existingProjection.recoveryDebugDetails ? existingProjection.recoveryDebugDetails : ''}`;
  const nextDebug = `${nextProjection && nextProjection.recoveryDebugDetails ? nextProjection.recoveryDebugDetails : ''}`;
  return existingState !== nextState || existingDebug !== nextDebug;
}
