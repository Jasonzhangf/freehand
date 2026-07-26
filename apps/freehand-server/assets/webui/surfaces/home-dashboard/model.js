export function createHomeDashboardModel({ activeSessions, historySessions, buckets }) {
  const running = Array.isArray(activeSessions) ? activeSessions : [];
  const history = Array.isArray(historySessions) ? historySessions : [];
  const historyBuckets = Array.isArray(buckets) ? buckets : [];
  return Object.freeze({
    running,
    history,
    buckets: historyBuckets,
    counts: Object.freeze({
      running: running.length,
      history: history.length,
      needsUser: history.filter((session) => /等待用户选择|blocked|阻塞/i.test(`${session?.latest_status || session?.status || ''}`)).length,
      blocked: history.filter((session) => /blocked|阻塞|failed|失败/i.test(`${session?.latest_status || session?.status || ''}`)).length,
    }),
  });
}
