use std::collections::HashMap;
use std::sync::Arc;

use freehand_v2_contracts::{
    CapabilityId, CorrelationId, EventId, ImmutablePayload, PluginId, SessionId, UiCommand,
};
use freehand_v2_control_events::{EventLedger, EventRecord};
use freehand_v2_cordis_ecosystem::{
    CordisContext, CordisError, PLUGIN_CONTRACT_VERSION, PluginRegistration, PluginRole,
};
use freehand_v2_plugin_capabilities::{CapabilityManifest, LocalCapabilityPlugin};
use freehand_v2_reasoning_backend::{
    NativeBackend, ReasoningError, ReasoningEvent, ReasoningRequest, ReasoningService,
    RuntimeGroupId,
};
use freehand_v2_sessionlog::{EventKind, SessionEvent, SessionLog, SurfaceOp};
use freehand_v2_ui_adaptor::{
    ProjectionKind, SlotId, UiAdaptor, UiCommandReceipt, UiError, UiProjection,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerticalSliceError {
    #[error("capability error: {0}")]
    Capability(String),
    #[error("reasoning error: {0}")]
    Reasoning(String),
    #[error("session log error: {0}")]
    SessionLog(String),
    #[error("ui error: {0}")]
    Ui(String),
    #[error("event id generation failed: {0}")]
    EventId(String),
    #[error("already terminal: {0}")]
    AlreadyTerminal(String),
    #[error("pending turn not found: {0}")]
    PendingTurnNotFound(String),
}

impl From<CordisError> for VerticalSliceError {
    fn from(error: CordisError) -> Self {
        Self::Capability(error.to_string())
    }
}

impl From<ReasoningError> for VerticalSliceError {
    fn from(error: ReasoningError) -> Self {
        Self::Reasoning(error.to_string())
    }
}

impl From<freehand_v2_sessionlog::SessionLogError> for VerticalSliceError {
    fn from(error: freehand_v2_sessionlog::SessionLogError) -> Self {
        Self::SessionLog(error.to_string())
    }
}

impl From<UiError> for VerticalSliceError {
    fn from(error: UiError) -> Self {
        Self::Ui(error.to_string())
    }
}

impl From<freehand_v2_contracts::ContractError> for VerticalSliceError {
    fn from(error: freehand_v2_contracts::ContractError) -> Self {
        Self::EventId(error.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct TurnOutcome {
    receipt: UiCommandReceipt,
    payload: Arc<ImmutablePayload>,
    projection: Option<UiProjection>,
    waiting: bool,
}

impl TurnOutcome {
    pub fn receipt(&self) -> &UiCommandReceipt {
        &self.receipt
    }

    pub fn payload_arc(&self) -> &Arc<ImmutablePayload> {
        &self.payload
    }

    pub fn projection(&self) -> &UiProjection {
        self.projection
            .as_ref()
            .expect("completed turn has a projection")
    }

    pub fn is_waiting(&self) -> bool {
        self.waiting
    }
}

struct PendingTurn {
    payload: Arc<ImmutablePayload>,
    session_id: SessionId,
}

pub struct PublicVerticalSlice {
    ui: UiAdaptor,
    cordis: CordisContext,
    sessionlog: SessionLog,
    reasoning: ReasoningService,
    run_slot: SlotId,
    group: RuntimeGroupId,
    pending: HashMap<CorrelationId, PendingTurn>,
    terminal: std::collections::HashSet<CorrelationId>,
}

impl PublicVerticalSlice {
    pub fn new() -> Self {
        Self::with_capability(false)
    }

    pub fn with_failing_capability() -> Self {
        Self::with_capability(true)
    }

    fn with_capability(fail_next: bool) -> Self {
        let mut cordis = CordisContext::new();
        register_role(
            &mut cordis,
            "design.orchestration",
            PluginRole::Orchestration,
        );
        register_role(&mut cordis, "events.local", PluginRole::ControlEvents);
        register_role(&mut cordis, "sessionlog.local", PluginRole::SessionLog);
        register_role(
            &mut cordis,
            "reasoning.native",
            PluginRole::ReasoningBackend,
        );
        register_role(&mut cordis, "capabilities.local", PluginRole::Capability);
        register_role(&mut cordis, "ui.adaptor", PluginRole::Ui);
        register_role(&mut cordis, "ui.run", PluginRole::Ui);
        register_role(&mut cordis, "notification.local", PluginRole::Notification);
        register_role(&mut cordis, "topology.local", PluginRole::Topology);
        register_role(&mut cordis, "session.canvas", PluginRole::SessionCanvas);
        register_role(&mut cordis, "search.local", PluginRole::Search);
        register_role(&mut cordis, "memory.local", PluginRole::Memory);
        register_role(&mut cordis, "channel.registry", PluginRole::Channel);
        register_role(&mut cordis, "network.reserved", PluginRole::NetworkReserved);

        let plugin_id = PluginId::try_new("capabilities.local").expect("plugin id");
        let capability_id = CapabilityId::try_new("local.capability").expect("capability id");
        let manifest = CapabilityManifest::try_new(
            plugin_id,
            capability_id,
            "string",
            "string",
            vec!["plugin.completed".to_owned()],
            vec!["local".to_owned()],
            None,
        )
        .expect("capability manifest");
        let plugin = make_local_capability(manifest, fail_next);
        cordis
            .register(Box::new(plugin))
            .expect("capability register");

        let mut reasoning = ReasoningService::new();
        let group = RuntimeGroupId::try_new("local").expect("runtime group");
        reasoning
            .bind(
                group.clone(),
                Box::new(NativeBackend::new().expect("native backend")),
            )
            .expect("bind backend");

        Self {
            ui: UiAdaptor::new(),
            cordis,
            sessionlog: SessionLog::new(),
            reasoning,
            run_slot: SlotId::try_new("run").expect("run slot"),
            group,
            pending: HashMap::new(),
            terminal: std::collections::HashSet::new(),
        }
    }

    pub fn submit(&mut self, command: UiCommand) -> Result<TurnOutcome, VerticalSliceError> {
        self.run_command(command, true)
    }

    pub fn begin(&mut self, command: UiCommand) -> Result<TurnOutcome, VerticalSliceError> {
        self.run_command(command, false)
    }

    pub fn resume(
        &mut self,
        correlation_id: &CorrelationId,
    ) -> Result<TurnOutcome, VerticalSliceError> {
        let pending = self.pending.remove(correlation_id).ok_or_else(|| {
            VerticalSliceError::PendingTurnNotFound(correlation_id.as_str().to_owned())
        })?;
        if self.terminal.contains(correlation_id) {
            return Err(VerticalSliceError::AlreadyTerminal(
                correlation_id.as_str().to_owned(),
            ));
        }

        let response = self
            .reasoning
            .subscribe(&pending.session_id, correlation_id)?;
        self.append_result(
            &pending.session_id,
            correlation_id,
            &response,
            &pending.payload,
        )?;
        let projection = self.publish_projection(&pending.session_id, &pending.payload)?;
        let outcome = TurnOutcome {
            receipt: self.accepted_receipt(correlation_id),
            payload: Arc::clone(&pending.payload),
            projection: Some(projection),
            waiting: false,
        };
        self.terminal.insert(correlation_id.clone());
        Ok(outcome)
    }

    pub fn session_events(&self, session_id: &SessionId) -> Vec<SessionEvent> {
        self.sessionlog
            .read_session(session_id)
            .map_or_else(|_| Vec::new(), |events| events.to_vec())
    }

    pub fn control_events(&self) -> Vec<EventRecord> {
        self.cordis.events().events().to_vec()
    }

    pub fn projection_count(&self, slot_id: &SlotId) -> usize {
        self.ui.projection_count(slot_id)
    }

    pub fn query_projection(&self, slot_id: &SlotId) -> Result<UiProjection, UiError> {
        self.ui.query(slot_id)
    }

    pub fn events_ledger(&self) -> &EventLedger {
        self.cordis.events()
    }

    pub fn registered_plugins(&self) -> Vec<PluginRegistration> {
        self.cordis.plugin_registrations()
    }

    fn run_command(
        &mut self,
        command: UiCommand,
        complete: bool,
    ) -> Result<TurnOutcome, VerticalSliceError> {
        let correlation_id = command.correlation_id().clone();
        if self.terminal.contains(&correlation_id) || self.pending.contains_key(&correlation_id) {
            return Err(VerticalSliceError::AlreadyTerminal(
                correlation_id.as_str().to_owned(),
            ));
        }

        let receipt = self.ui.accept_command(self.run_slot.clone(), &command)?;
        let session_id = command.session_id().clone();
        let capability_id = command.capability_id().clone();
        let payload = Arc::new(command.payload().clone());

        if self.session_events(&session_id).is_empty() {
            self.sessionlog
                .create_session(session_id.clone(), 1, Some("m8-local".to_owned()))?;
        }

        self.append_input(&session_id, &correlation_id, payload.body())?;
        let invocation =
            self.cordis
                .invoke(correlation_id.clone(), capability_id, Arc::clone(&payload))?;
        if !invocation.invocation().success() {
            return Err(VerticalSliceError::Capability(
                "capability invocation returned success=false".to_owned(),
            ));
        }

        self.append_surface(&session_id, &correlation_id, payload.body())?;
        let request = ReasoningRequest::new(
            session_id.clone(),
            correlation_id.clone(),
            Arc::clone(&payload),
            None,
        );
        self.reasoning.start(&self.group, request)?;

        if !complete {
            self.pending.insert(
                correlation_id.clone(),
                PendingTurn {
                    payload: Arc::clone(&payload),
                    session_id,
                },
            );
            return Ok(TurnOutcome {
                receipt,
                payload,
                projection: None,
                waiting: true,
            });
        }

        let response = self.reasoning.subscribe(&session_id, &correlation_id)?;
        self.append_result(&session_id, &correlation_id, &response, &payload)?;
        let projection = self.publish_projection(&session_id, &payload)?;
        self.terminal.insert(correlation_id);
        Ok(TurnOutcome {
            receipt,
            payload,
            projection: Some(projection),
            waiting: false,
        })
    }

    fn accepted_receipt(&self, correlation_id: &CorrelationId) -> UiCommandReceipt {
        UiCommandReceipt::new(
            format!("cmd-receipt-{}", correlation_id.as_str()),
            self.run_slot.clone(),
            freehand_v2_ui_adaptor::UiCommandReceiptStatus::Accepted,
            "command accepted",
        )
    }

    fn append_input(
        &mut self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
        payload_body: &str,
    ) -> Result<(), VerticalSliceError> {
        let event_id = EventId::try_new(format!("m8-input-{}", correlation_id.as_str()))?;
        self.sessionlog.append_event(
            session_id,
            event_id,
            1,
            EventKind::Input,
            format!("user:{payload_body}"),
            Some(SurfaceOp::Replace),
            Vec::new(),
            false,
        )?;
        Ok(())
    }

    fn append_surface(
        &mut self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
        payload_body: &str,
    ) -> Result<(), VerticalSliceError> {
        let event_id = EventId::try_new(format!("m8-surface-{}", correlation_id.as_str()))?;
        self.sessionlog.append_event(
            session_id,
            event_id,
            2,
            EventKind::Surface,
            format!("surface:{payload_body}"),
            Some(SurfaceOp::Replace),
            Vec::new(),
            false,
        )?;
        Ok(())
    }

    fn append_result(
        &mut self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
        _reasoning_event: &ReasoningEvent,
        payload: &Arc<ImmutablePayload>,
    ) -> Result<(), VerticalSliceError> {
        let event_id = EventId::try_new(format!("m8-result-{}", correlation_id.as_str()))?;
        self.sessionlog.append_event(
            session_id,
            event_id,
            3,
            EventKind::Result,
            format!("{}:result", payload.body()),
            Some(SurfaceOp::Replace),
            Vec::new(),
            true,
        )?;
        Ok(())
    }

    fn publish_projection(
        &mut self,
        session_id: &SessionId,
        payload: &Arc<ImmutablePayload>,
    ) -> Result<UiProjection, VerticalSliceError> {
        let result_payload = ImmutablePayload::new(format!("{}:result", payload.body()))?;
        Ok(self.ui.publish_projection(
            self.run_slot.clone(),
            ProjectionKind::Run,
            format!("sessionlog:{}", session_id.as_str()),
            Arc::new(result_payload),
        )?)
    }
}

impl Default for PublicVerticalSlice {
    fn default() -> Self {
        Self::new()
    }
}

fn make_local_capability(manifest: CapabilityManifest, fail_next: bool) -> LocalCapabilityPlugin {
    let plugin = LocalCapabilityPlugin::new(manifest).expect("local plugin");
    if fail_next {
        plugin.fail_next()
    } else {
        plugin
    }
}

fn register_role(cordis: &mut CordisContext, plugin_id: &str, role: PluginRole) {
    let registration = PluginRegistration::try_new(
        PluginId::try_new(plugin_id).expect("plugin id"),
        role,
        PLUGIN_CONTRACT_VERSION,
    )
    .expect("plugin registration");
    cordis
        .register_plugin(registration)
        .expect("register plugin");
}
