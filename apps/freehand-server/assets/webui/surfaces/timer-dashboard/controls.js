export async function openTimerDashboardSurface(context) {
  context.dispatchEdge('root.open_timer');
  if (context.dom.dialog && typeof context.dom.dialog.showModal === 'function' && !context.dom.dialog.open) {
    context.dom.dialog.showModal();
  }
  context.renderTimerDashboard();
  await context.refreshTimerDashboard();
}

export async function refreshTimerDashboardSurface(context) {
  context.dispatchEdge('timer.refresh');
  try {
    const result = await context.adpQuery({ QueryTimerList: { include_terminal: true } });
    context.applyPhase2QueryResult(result);
    context.setCommandStatus('定时器投影已刷新。');
  } catch (error) {
    context.state.timerStatusError = error.message;
    context.setCommandStatus(`定时器刷新失败：${error.message}`, { stickyMs: 9000 });
    context.renderTimerDashboard();
    context.renderMobileHomeDashboard();
  }
}

export async function scheduleTimerFromSurface(context) {
  const timer = context.buildTimerSchedulePayload();
  context.state.timerCommandInFlight = true;
  context.renderTimerDashboard();
  try {
    const receipt = await context.adpCommand({ ScheduleTimer: { timer } });
    const message = context.timerScheduleReceiptStatus(receipt);
    context.setCommandStatus(message, { stickyMs: 8000 });
    await context.refreshTimerDashboard();
  } catch (error) {
    context.state.timerStatusError = error.message;
    context.setCommandStatus(`定时创建失败：${error.message}`, { stickyMs: 9000 });
  } finally {
    context.state.timerCommandInFlight = false;
    context.renderTimerDashboard();
    context.renderMobileHomeDashboard();
  }
}

export async function cancelTimerFromSurface(context, timerId) {
  if (!timerId) return;
  context.state.timerCommandInFlight = true;
  context.renderTimerDashboard();
  try {
    const receipt = await context.adpCommand({ CancelTimer: { timer_id: timerId } });
    const message = context.timerCancelReceiptStatus(receipt);
    context.setCommandStatus(message, { stickyMs: 8000 });
    await context.refreshTimerDashboard();
  } catch (error) {
    context.state.timerStatusError = error.message;
    context.setCommandStatus(`定时取消失败：${error.message}`, { stickyMs: 9000 });
  } finally {
    context.state.timerCommandInFlight = false;
    context.renderTimerDashboard();
    context.renderMobileHomeDashboard();
  }
}
