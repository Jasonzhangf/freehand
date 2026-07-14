use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use freehand_contracts::AgentId;
use freehand_task::{
    AGENT_PROCESS_HEARTBEAT_TTL_SECONDS, AgentLifecycleEvent, TaskHeartbeatRequest, TaskId,
    TaskRuntime,
};

use super::{
    DEFAULT_LEASE_TTL_SECONDS, ProductionWorkerRunnerError, WorkerProcessIdentity,
    now_unix_seconds, worker_actor, worker_watermark,
};

pub(super) struct WorkerHeartbeat {
    stop: mpsc::Sender<()>,
    error: Arc<Mutex<Option<String>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl WorkerHeartbeat {
    pub(super) fn start(
        task_runtime: Arc<TaskRuntime>,
        task_id: TaskId,
        worker_agent_id: AgentId,
        execution_id: String,
        process_identity: WorkerProcessIdentity,
    ) -> Self {
        let (stop, receiver) = mpsc::channel();
        let error = Arc::new(Mutex::new(None));
        let thread_error = Arc::clone(&error);
        let interval = Duration::from_secs(
            (DEFAULT_LEASE_TTL_SECONDS / 3).clamp(1, AGENT_PROCESS_HEARTBEAT_TTL_SECONDS / 2),
        );
        let handle = thread::spawn(move || {
            while receiver.recv_timeout(interval).is_err() {
                let result = task_runtime.heartbeat_task(TaskHeartbeatRequest {
                    task_id: task_id.clone(),
                    ttl_seconds: DEFAULT_LEASE_TTL_SECONDS,
                    actor: worker_actor(&worker_agent_id, None),
                    watermark: worker_watermark(&execution_id, "heartbeat"),
                });
                if let Err(error) = result {
                    *thread_error.lock().expect("lock worker heartbeat error") =
                        Some(error.to_string());
                    break;
                }
                let result = task_runtime.apply_agent_lifecycle_event(
                    AgentLifecycleEvent::ProcessHeartbeat {
                        agent_id: worker_agent_id.clone(),
                        process_id: process_identity.process_id,
                        process_instance_id: process_identity.process_instance_id.clone(),
                        observed_at: now_unix_seconds(),
                    },
                );
                if let Err(error) = result {
                    *thread_error.lock().expect("lock worker heartbeat error") =
                        Some(error.to_string());
                    break;
                }
            }
        });
        Self {
            stop,
            error,
            handle: Some(handle),
        }
    }

    pub(super) fn stop(mut self) -> Result<(), ProductionWorkerRunnerError> {
        let _ = self.stop.send(());
        if let Some(handle) = self.handle.take() {
            handle.join().map_err(|_| {
                ProductionWorkerRunnerError::Heartbeat(
                    "worker heartbeat thread panicked".to_owned(),
                )
            })?;
        }
        if let Some(error) = self
            .error
            .lock()
            .expect("lock worker heartbeat error")
            .take()
        {
            return Err(ProductionWorkerRunnerError::Heartbeat(error));
        }
        Ok(())
    }
}

impl Drop for WorkerHeartbeat {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
