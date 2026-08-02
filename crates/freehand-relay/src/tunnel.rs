use std::collections::BTreeMap;

use tokio::sync::{mpsc, oneshot};

use crate::model::{RelayDataFrameKind, RelayDataInFrame, RelayDataOutFrame, RelayErrorOutFrame};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayResponseOpen {
    pub status: Option<u16>,
    pub headers: Vec<(String, Vec<u8>)>,
}

#[derive(Debug)]
pub enum RelayResponsePart {
    Chunk {
        frame_kind: Option<RelayDataFrameKind>,
        bytes: Vec<u8>,
    },
    End,
}

#[derive(Debug)]
pub struct RelayPendingResponse {
    pub open: oneshot::Receiver<Result<RelayResponseOpen, String>>,
    pub parts: mpsc::Receiver<Result<RelayResponsePart, String>>,
}

#[derive(Debug)]
pub struct RelayRoutableExchange {
    pub data_sender: RelayDataTunnelSender,
    pub error_sender: RelayErrorTunnelSender,
    pub pending: RelayPendingResponse,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RelayExchangeAdmissionError {
    #[error("Relay data tunnel is not attached")]
    DataTunnelUnavailable,
    #[error("Relay error tunnel is not attached")]
    ErrorTunnelUnavailable,
    #[error("Relay exchange admission failed: {0}")]
    Invalid(String),
}

#[derive(Debug)]
pub enum RelayDataDelivery {
    Cancelled,
    Open {
        sender: oneshot::Sender<Result<RelayResponseOpen, String>>,
        response: RelayResponseOpen,
    },
    Part {
        sender: mpsc::Sender<Result<RelayResponsePart, String>>,
        part: Result<RelayResponsePart, String>,
    },
}

impl RelayDataDelivery {
    pub async fn deliver(self) -> Result<(), String> {
        match self {
            Self::Cancelled => Ok(()),
            Self::Open { sender, response } => sender
                .send(Ok(response))
                .map_err(|_| "Relay response receiver is closed".to_owned()),
            Self::Part { sender, part } => sender
                .send(part)
                .await
                .map_err(|_| "Relay response receiver is closed".to_owned()),
        }
    }
}

#[derive(Debug)]
struct RelayPendingSender {
    identity: RelayTunnelIdentity,
    cancelled: bool,
    open: Option<oneshot::Sender<Result<RelayResponseOpen, String>>>,
    parts: mpsc::Sender<Result<RelayResponsePart, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelayTunnelIdentity {
    pub account_id: String,
    pub agent_id: String,
}

#[derive(Debug, Clone)]
pub struct RelayDataTunnelSender {
    sender: mpsc::Sender<RelayDataOutFrame>,
}

#[derive(Debug, Clone)]
pub struct RelayErrorTunnelSender {
    sender: mpsc::Sender<RelayErrorOutFrame>,
}

impl RelayErrorTunnelSender {
    pub fn new(sender: mpsc::Sender<RelayErrorOutFrame>) -> Self {
        Self { sender }
    }

    pub async fn send(&self, frame: RelayErrorOutFrame) -> Result<(), String> {
        self.sender
            .send(frame)
            .await
            .map_err(|_| "Relay error tunnel is closed".to_owned())
    }
}

#[derive(Debug, Clone)]
struct RelayDataTunnelAttachment {
    generation: u64,
    sender: RelayDataTunnelSender,
}

#[derive(Debug, Clone)]
struct RelayErrorTunnelAttachment {
    generation: u64,
    sender: RelayErrorTunnelSender,
}

impl RelayDataTunnelSender {
    pub fn new(sender: mpsc::Sender<RelayDataOutFrame>) -> Self {
        Self { sender }
    }

    pub async fn send(&self, frame: RelayDataOutFrame) -> Result<(), String> {
        self.sender
            .send(frame)
            .await
            .map_err(|_| "Relay data tunnel is closed".to_owned())
    }
}

#[derive(Debug, Default)]
pub struct RelayTunnelRegistry {
    control_channels: BTreeMap<RelayTunnelIdentity, u64>,
    data_channels: BTreeMap<RelayTunnelIdentity, RelayDataTunnelAttachment>,
    error_channels: BTreeMap<RelayTunnelIdentity, RelayErrorTunnelAttachment>,
    pending: BTreeMap<String, RelayPendingSender>,
    next_control_generation: u64,
    next_data_generation: u64,
    next_error_generation: u64,
}

impl RelayTunnelRegistry {
    pub fn attach_control(&mut self, identity: RelayTunnelIdentity) -> Result<u64, String> {
        if self.control_channels.contains_key(&identity) {
            return Err("Relay control tunnel is already attached".to_owned());
        }
        self.next_control_generation = self.next_control_generation.wrapping_add(1);
        let generation = self.next_control_generation;
        self.control_channels.insert(identity, generation);
        Ok(generation)
    }

    pub fn detach_control(&mut self, identity: &RelayTunnelIdentity, generation: u64) -> bool {
        if self.control_channels.get(identity).copied() != Some(generation) {
            return false;
        }
        self.control_channels.remove(identity).is_some()
    }

    pub fn has_control(&self, identity: &RelayTunnelIdentity) -> bool {
        self.control_channels.contains_key(identity)
    }

    pub fn attach_data(
        &mut self,
        identity: RelayTunnelIdentity,
        sender: RelayDataTunnelSender,
    ) -> Result<u64, String> {
        if self.data_channels.contains_key(&identity) {
            return Err("Relay data tunnel is already attached".to_owned());
        }
        self.next_data_generation = self.next_data_generation.wrapping_add(1);
        let generation = self.next_data_generation;
        self.data_channels
            .insert(identity, RelayDataTunnelAttachment { generation, sender });
        Ok(generation)
    }

    pub fn detach_data(
        &mut self,
        identity: &RelayTunnelIdentity,
        generation: u64,
    ) -> Result<Vec<RelayDataDelivery>, String> {
        if self
            .data_channels
            .get(identity)
            .is_some_and(|attachment| attachment.generation != generation)
        {
            return Ok(Vec::new());
        }
        self.detach_current_data(identity)
    }

    pub fn detach_current_data(
        &mut self,
        identity: &RelayTunnelIdentity,
    ) -> Result<Vec<RelayDataDelivery>, String> {
        self.data_channels.remove(identity);
        let exchange_ids = self
            .pending
            .iter()
            .filter_map(|(exchange_id, pending)| {
                (&pending.identity == identity).then_some(exchange_id.clone())
            })
            .collect::<Vec<_>>();
        let mut deliveries = Vec::new();
        for exchange_id in exchange_ids {
            if let Some(delivery) = self.fail_exchange(
                identity,
                &exchange_id,
                "Relay data tunnel disconnected".to_owned(),
            )? {
                deliveries.push(delivery);
            }
        }
        Ok(deliveries)
    }

    pub fn data_sender(&self, identity: &RelayTunnelIdentity) -> Option<RelayDataTunnelSender> {
        self.data_channels
            .get(identity)
            .map(|attachment| attachment.sender.clone())
    }

    pub(crate) fn admit_error(
        &mut self,
        identity: RelayTunnelIdentity,
        sender: RelayErrorTunnelSender,
    ) -> Result<u64, String> {
        if !self.has_control(&identity) {
            return Err("Relay error tunnel requires an active control tunnel".to_owned());
        }
        if self.error_channels.contains_key(&identity) {
            return Err("Relay error tunnel is already attached".to_owned());
        }
        self.next_error_generation = self.next_error_generation.wrapping_add(1);
        let generation = self.next_error_generation;
        self.error_channels
            .insert(identity, RelayErrorTunnelAttachment { generation, sender });
        Ok(generation)
    }

    pub fn detach_error(&mut self, identity: &RelayTunnelIdentity, generation: u64) -> bool {
        if self
            .error_channels
            .get(identity)
            .is_some_and(|attachment| attachment.generation != generation)
        {
            return false;
        }
        self.error_channels.remove(identity).is_some()
    }

    pub fn detach_current_error(&mut self, identity: &RelayTunnelIdentity) {
        self.error_channels.remove(identity);
    }

    pub fn error_sender(&self, identity: &RelayTunnelIdentity) -> Option<RelayErrorTunnelSender> {
        self.error_channels
            .get(identity)
            .map(|attachment| attachment.sender.clone())
    }

    #[cfg(test)]
    pub(crate) fn has_pending_exchange(&self, exchange_id: &str) -> bool {
        self.pending.contains_key(exchange_id)
    }

    fn open_exchange(
        &mut self,
        identity: RelayTunnelIdentity,
        exchange_id: String,
    ) -> Result<RelayPendingResponse, String> {
        if self.pending.contains_key(&exchange_id) {
            return Err("Relay exchange id is already active".to_owned());
        }
        let (open_tx, open_rx) = oneshot::channel();
        let (parts_tx, parts_rx) = mpsc::channel(32);
        self.pending.insert(
            exchange_id,
            RelayPendingSender {
                identity,
                cancelled: false,
                open: Some(open_tx),
                parts: parts_tx,
            },
        );
        Ok(RelayPendingResponse {
            open: open_rx,
            parts: parts_rx,
        })
    }

    #[cfg(test)]
    pub(crate) fn open_exchange_for_test(
        &mut self,
        identity: RelayTunnelIdentity,
        exchange_id: String,
    ) -> Result<RelayPendingResponse, String> {
        self.open_exchange(identity, exchange_id)
    }

    pub fn open_routable_exchange(
        &mut self,
        identity: RelayTunnelIdentity,
        exchange_id: String,
    ) -> Result<RelayRoutableExchange, RelayExchangeAdmissionError> {
        let data_sender = self
            .data_sender(&identity)
            .ok_or(RelayExchangeAdmissionError::DataTunnelUnavailable)?;
        let error_sender = self
            .error_sender(&identity)
            .ok_or(RelayExchangeAdmissionError::ErrorTunnelUnavailable)?;
        let pending = self
            .open_exchange(identity, exchange_id)
            .map_err(RelayExchangeAdmissionError::Invalid)?;
        Ok(RelayRoutableExchange {
            data_sender,
            error_sender,
            pending,
        })
    }

    pub fn accept_data(
        &mut self,
        identity: &RelayTunnelIdentity,
        frame: RelayDataInFrame,
    ) -> Result<RelayDataDelivery, String> {
        match frame {
            RelayDataInFrame::ResponseOpen {
                exchange_id,
                status,
                headers,
            } => {
                let pending = self
                    .pending
                    .get_mut(&exchange_id)
                    .ok_or_else(|| "Relay response references an unknown exchange".to_owned())?;
                require_pending_identity(pending, identity)?;
                if pending.cancelled {
                    return Ok(RelayDataDelivery::Cancelled);
                }
                let sender = pending
                    .open
                    .take()
                    .ok_or_else(|| "Relay response opened the exchange twice".to_owned())?;
                Ok(RelayDataDelivery::Open {
                    sender,
                    response: RelayResponseOpen { status, headers },
                })
            }
            RelayDataInFrame::ResponseChunk {
                exchange_id,
                frame_kind,
                bytes,
            } => {
                let pending = self
                    .pending
                    .get(&exchange_id)
                    .ok_or_else(|| "Relay response references an unknown exchange".to_owned())?;
                require_pending_identity(pending, identity)?;
                if pending.cancelled {
                    return Ok(RelayDataDelivery::Cancelled);
                }
                Ok(RelayDataDelivery::Part {
                    sender: pending.parts.clone(),
                    part: Ok(RelayResponsePart::Chunk { frame_kind, bytes }),
                })
            }
            RelayDataInFrame::ResponseEnd { exchange_id } => {
                let pending = self
                    .pending
                    .get(&exchange_id)
                    .ok_or_else(|| "Relay response references an unknown exchange".to_owned())?;
                require_pending_identity(pending, identity)?;
                let pending = self
                    .pending
                    .remove(&exchange_id)
                    .ok_or_else(|| "Relay response references an unknown exchange".to_owned())?;
                if pending.cancelled {
                    return Ok(RelayDataDelivery::Cancelled);
                }
                Ok(RelayDataDelivery::Part {
                    sender: pending.parts,
                    part: Ok(RelayResponsePart::End),
                })
            }
        }
    }

    pub fn fail_exchange(
        &mut self,
        identity: &RelayTunnelIdentity,
        exchange_id: &str,
        message: String,
    ) -> Result<Option<RelayDataDelivery>, String> {
        let pending = self
            .pending
            .get(exchange_id)
            .ok_or_else(|| "Relay error references an unknown exchange".to_owned())?;
        require_pending_identity(pending, identity)?;
        if pending.cancelled {
            self.pending.remove(exchange_id);
            return Ok(None);
        }
        let mut pending = self
            .pending
            .remove(exchange_id)
            .ok_or_else(|| "Relay error references an unknown exchange".to_owned())?;
        if let Some(open) = pending.open.take() {
            open.send(Err(message))
                .map_err(|_| "Relay response receiver is closed".to_owned())?;
            Ok(None)
        } else {
            Ok(Some(RelayDataDelivery::Part {
                sender: pending.parts,
                part: Err(message),
            }))
        }
    }

    pub fn cancel_exchange(
        &mut self,
        identity: &RelayTunnelIdentity,
        exchange_id: &str,
    ) -> Result<bool, String> {
        let Some(pending) = self.pending.get_mut(exchange_id) else {
            return Ok(false);
        };
        require_pending_identity(pending, identity)?;
        pending.cancelled = true;
        Ok(true)
    }
}

fn require_pending_identity(
    pending: &RelayPendingSender,
    identity: &RelayTunnelIdentity,
) -> Result<(), String> {
    if &pending.identity != identity {
        return Err("Relay frame identity does not own the exchange".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> RelayTunnelIdentity {
        RelayTunnelIdentity {
            account_id: "account-1".to_owned(),
            agent_id: "agent-1".to_owned(),
        }
    }

    #[test]
    fn routable_exchange_requires_data_and_error_before_pending_insertion() {
        let identity = identity();
        let mut registry = RelayTunnelRegistry::default();
        assert_eq!(
            registry
                .open_routable_exchange(identity.clone(), "missing-data".to_owned())
                .expect_err("missing data must reject"),
            RelayExchangeAdmissionError::DataTunnelUnavailable
        );
        assert!(!registry.has_pending_exchange("missing-data"));

        let (data_tx, _data_rx) = mpsc::channel(1);
        registry
            .attach_data(identity.clone(), RelayDataTunnelSender::new(data_tx))
            .expect("data attachment");
        assert_eq!(
            registry
                .open_routable_exchange(identity.clone(), "missing-error".to_owned())
                .expect_err("missing error must reject"),
            RelayExchangeAdmissionError::ErrorTunnelUnavailable
        );
        assert!(!registry.has_pending_exchange("missing-error"));

        let (error_tx, _error_rx) = mpsc::channel(1);
        registry
            .attach_control(identity.clone())
            .expect("control attachment");
        registry
            .admit_error(identity.clone(), RelayErrorTunnelSender::new(error_tx))
            .expect("error attachment");
        registry
            .open_routable_exchange(identity, "routable".to_owned())
            .expect("complete route must open");
        assert!(registry.has_pending_exchange("routable"));
    }

    #[tokio::test]
    async fn successful_exchange_remains_pending_until_response_end() {
        let mut registry = RelayTunnelRegistry::default();
        let mut pending = registry
            .open_exchange(identity(), "exchange-1".to_owned())
            .expect("exchange must open");

        registry
            .accept_data(
                &identity(),
                RelayDataInFrame::ResponseOpen {
                    exchange_id: "exchange-1".to_owned(),
                    status: Some(200),
                    headers: Vec::new(),
                },
            )
            .expect("response open must be accepted")
            .deliver()
            .await
            .expect("response open must be delivered");
        assert!(registry.pending.contains_key("exchange-1"));
        registry
            .accept_data(
                &identity(),
                RelayDataInFrame::ResponseChunk {
                    exchange_id: "exchange-1".to_owned(),
                    frame_kind: None,
                    bytes: b"ok".to_vec(),
                },
            )
            .expect("response chunk must be accepted")
            .deliver()
            .await
            .expect("response chunk must be delivered");
        assert!(registry.pending.contains_key("exchange-1"));
        registry
            .accept_data(
                &identity(),
                RelayDataInFrame::ResponseEnd {
                    exchange_id: "exchange-1".to_owned(),
                },
            )
            .expect("response end must be accepted")
            .deliver()
            .await
            .expect("response end must be delivered");
        assert!(!registry.pending.contains_key("exchange-1"));

        assert_eq!(
            pending.open.await.expect("open receiver must remain live"),
            Ok(RelayResponseOpen {
                status: Some(200),
                headers: Vec::new(),
            })
        );
        assert!(matches!(
            pending.parts.recv().await,
            Some(Ok(RelayResponsePart::Chunk { bytes, .. })) if bytes == b"ok"
        ));
        assert!(matches!(
            pending.parts.recv().await,
            Some(Ok(RelayResponsePart::End))
        ));
    }

    #[tokio::test]
    async fn cancelled_exchange_absorbs_queued_data_until_response_end() {
        let mut registry = RelayTunnelRegistry::default();
        let _pending = registry
            .open_exchange(identity(), "cancelled-exchange".to_owned())
            .expect("exchange must open");
        assert!(
            registry
                .cancel_exchange(&identity(), "cancelled-exchange")
                .expect("cancel exchange")
        );
        assert!(matches!(
            registry
                .accept_data(
                    &identity(),
                    RelayDataInFrame::ResponseChunk {
                        exchange_id: "cancelled-exchange".to_owned(),
                        frame_kind: None,
                        bytes: b"queued-before-cancel".to_vec(),
                    },
                )
                .expect("queued chunk remains correlated"),
            RelayDataDelivery::Cancelled
        ));
        assert!(registry.has_pending_exchange("cancelled-exchange"));
        assert!(matches!(
            registry
                .accept_data(
                    &identity(),
                    RelayDataInFrame::ResponseEnd {
                        exchange_id: "cancelled-exchange".to_owned(),
                    },
                )
                .expect("cancellation response end"),
            RelayDataDelivery::Cancelled
        ));
        assert!(!registry.has_pending_exchange("cancelled-exchange"));
    }

    #[tokio::test]
    async fn response_delivery_applies_backpressure_without_rejecting_valid_chunks() {
        let mut registry = RelayTunnelRegistry::default();
        let mut pending = registry
            .open_exchange(identity(), "exchange-backpressure".to_owned())
            .expect("exchange must open");
        registry
            .accept_data(
                &identity(),
                RelayDataInFrame::ResponseOpen {
                    exchange_id: "exchange-backpressure".to_owned(),
                    status: Some(200),
                    headers: Vec::new(),
                },
            )
            .expect("response open must be accepted")
            .deliver()
            .await
            .expect("response open must be delivered");
        pending
            .open
            .await
            .expect("response open receiver")
            .expect("response open");

        let mut deliveries = Vec::new();
        for index in 0..40u8 {
            deliveries.push(
                registry
                    .accept_data(
                        &identity(),
                        RelayDataInFrame::ResponseChunk {
                            exchange_id: "exchange-backpressure".to_owned(),
                            frame_kind: None,
                            bytes: vec![index],
                        },
                    )
                    .expect("valid chunk must be accepted before delivery capacity is available"),
            );
        }
        let delivery_task = tokio::spawn(async move {
            for delivery in deliveries {
                delivery.deliver().await.expect("chunk delivery");
            }
        });
        for index in 0..40u8 {
            assert!(matches!(
                pending.parts.recv().await,
                Some(Ok(RelayResponsePart::Chunk { bytes, .. })) if bytes == vec![index]
            ));
        }
        delivery_task.await.expect("delivery task");
    }

    #[tokio::test]
    async fn terminal_error_waits_for_response_capacity_and_preserves_error() {
        let mut registry = RelayTunnelRegistry::default();
        let mut pending = registry
            .open_exchange(identity(), "exchange-error-backpressure".to_owned())
            .expect("exchange must open");
        registry
            .accept_data(
                &identity(),
                RelayDataInFrame::ResponseOpen {
                    exchange_id: "exchange-error-backpressure".to_owned(),
                    status: Some(200),
                    headers: Vec::new(),
                },
            )
            .expect("response open")
            .deliver()
            .await
            .expect("response open delivery");
        pending.open.await.expect("open receiver").expect("open");
        for index in 0..32u8 {
            registry
                .accept_data(
                    &identity(),
                    RelayDataInFrame::ResponseChunk {
                        exchange_id: "exchange-error-backpressure".to_owned(),
                        frame_kind: None,
                        bytes: vec![index],
                    },
                )
                .expect("chunk")
                .deliver()
                .await
                .expect("fill response capacity");
        }
        let error_delivery = registry
            .fail_exchange(
                &identity(),
                "exchange-error-backpressure",
                "bridge failed after open".to_owned(),
            )
            .expect("active exchange failure")
            .expect("post-open failure delivery");
        assert!(!registry.pending.contains_key("exchange-error-backpressure"));

        let mut delivery_task = tokio::spawn(error_delivery.deliver());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut delivery_task)
                .await
                .is_err(),
            "terminal error delivery bypassed bounded response capacity"
        );
        let _ = pending.parts.recv().await.expect("first buffered chunk");
        delivery_task
            .await
            .expect("delivery task")
            .expect("error delivery");
        for _ in 1..32 {
            let _ = pending.parts.recv().await.expect("buffered chunk");
        }
        assert!(matches!(
            pending.parts.recv().await,
            Some(Err(error)) if error == "bridge failed after open"
        ));
    }

    #[tokio::test]
    async fn failure_removes_only_the_active_exchange_and_unknown_failure_is_explicit() {
        let mut registry = RelayTunnelRegistry::default();
        let pending = registry
            .open_exchange(identity(), "exchange-1".to_owned())
            .expect("exchange must open");

        registry
            .fail_exchange(&identity(), "exchange-1", "bridge failed".to_owned())
            .expect("active exchange must fail");
        assert!(!registry.pending.contains_key("exchange-1"));
        assert_eq!(
            pending.open.await.expect("failure must reach receiver"),
            Err("bridge failed".to_owned())
        );
        assert_eq!(
            registry
                .fail_exchange(&identity(), "exchange-1", "late failure".to_owned())
                .expect_err("already-terminal exchange must fail explicitly"),
            "Relay error references an unknown exchange"
        );
    }

    #[test]
    fn response_and_error_frames_cannot_cross_tunnel_identity() {
        let mut registry = RelayTunnelRegistry::default();
        let owner = identity();
        let foreign = RelayTunnelIdentity {
            account_id: "account-2".to_owned(),
            agent_id: "agent-2".to_owned(),
        };
        let _pending = registry
            .open_exchange(owner, "exchange-1".to_owned())
            .expect("exchange must open");

        assert_eq!(
            registry
                .accept_data(
                    &foreign,
                    RelayDataInFrame::ResponseOpen {
                        exchange_id: "exchange-1".to_owned(),
                        status: Some(200),
                        headers: Vec::new(),
                    },
                )
                .expect_err("foreign response must be rejected"),
            "Relay frame identity does not own the exchange"
        );
        assert_eq!(
            registry
                .fail_exchange(&foreign, "exchange-1", "foreign failure".to_owned())
                .expect_err("foreign error must be rejected"),
            "Relay frame identity does not own the exchange"
        );
        assert!(registry.pending.contains_key("exchange-1"));
    }

    #[tokio::test]
    async fn data_disconnect_fails_every_matching_pending_exchange() {
        let mut registry = RelayTunnelRegistry::default();
        let identity = identity();
        let first = registry
            .open_exchange(identity.clone(), "exchange-1".to_owned())
            .expect("first exchange must open");
        let second = registry
            .open_exchange(identity.clone(), "exchange-2".to_owned())
            .expect("second exchange must open");

        registry
            .detach_current_data(&identity)
            .expect("disconnect cleanup must close all exchanges");
        assert!(registry.pending.is_empty());
        assert_eq!(
            first.open.await.expect("first failure must arrive"),
            Err("Relay data tunnel disconnected".to_owned())
        );
        assert_eq!(
            second.open.await.expect("second failure must arrive"),
            Err("Relay data tunnel disconnected".to_owned())
        );
    }

    #[test]
    fn stale_data_generation_cannot_detach_reconnected_tunnel() {
        let mut registry = RelayTunnelRegistry::default();
        let identity = identity();
        let (first_tx, _first_rx) = mpsc::channel(1);
        let first = registry
            .attach_data(identity.clone(), RelayDataTunnelSender::new(first_tx))
            .expect("first data tunnel");
        registry
            .detach_data(&identity, first)
            .expect("first disconnect");
        let (second_tx, _second_rx) = mpsc::channel(1);
        let second = registry
            .attach_data(identity.clone(), RelayDataTunnelSender::new(second_tx))
            .expect("second data tunnel");

        registry
            .detach_data(&identity, first)
            .expect("stale disconnect is idempotent");
        assert!(registry.data_sender(&identity).is_some());
        registry
            .detach_data(&identity, second)
            .expect("current disconnect");
        assert!(registry.data_sender(&identity).is_none());
    }

    #[test]
    fn duplicate_and_stale_control_cleanup_cannot_detach_current_tunnel() {
        let mut registry = RelayTunnelRegistry::default();
        let identity = identity();
        let first = registry
            .attach_control(identity.clone())
            .expect("first control tunnel");
        assert_eq!(
            registry
                .attach_control(identity.clone())
                .expect_err("duplicate control tunnel must fail"),
            "Relay control tunnel is already attached"
        );
        assert!(!registry.detach_control(&identity, first.wrapping_add(1)));
        assert!(registry.has_control(&identity));
        assert!(registry.detach_control(&identity, first));
        assert!(!registry.has_control(&identity));
    }

    #[tokio::test]
    async fn duplicate_and_stale_error_attachments_cannot_replace_current_tunnel() {
        let mut registry = RelayTunnelRegistry::default();
        let identity = identity();
        registry
            .attach_control(identity.clone())
            .expect("control tunnel");
        let (first_tx, mut first_rx) = mpsc::channel(1);
        let first = registry
            .admit_error(identity.clone(), RelayErrorTunnelSender::new(first_tx))
            .expect("first error tunnel");
        let (duplicate_tx, mut duplicate_rx) = mpsc::channel(1);
        assert_eq!(
            registry
                .admit_error(identity.clone(), RelayErrorTunnelSender::new(duplicate_tx))
                .expect_err("duplicate error tunnel must fail"),
            "Relay error tunnel is already attached"
        );
        let frame = RelayErrorOutFrame::Terminal {
            code: "first-current".to_owned(),
            message: "first attachment remains current".to_owned(),
        };
        registry
            .error_sender(&identity)
            .expect("current error sender")
            .send(frame.clone())
            .await
            .expect("current error delivery");
        assert_eq!(first_rx.recv().await, Some(frame));
        assert_eq!(duplicate_rx.recv().await, None);

        assert!(registry.detach_error(&identity, first));
        let (second_tx, _second_rx) = mpsc::channel(1);
        let second = registry
            .admit_error(identity.clone(), RelayErrorTunnelSender::new(second_tx))
            .expect("second error tunnel");
        assert!(!registry.detach_error(&identity, first));
        assert!(registry.error_sender(&identity).is_some());
        assert!(registry.detach_error(&identity, second));
        assert!(registry.error_sender(&identity).is_none());
    }

    #[test]
    fn error_attachment_requires_control_in_the_same_registry_mutation() {
        let mut registry = RelayTunnelRegistry::default();
        let identity = identity();
        let (rejected_tx, _rejected_rx) = mpsc::channel(1);
        assert_eq!(
            registry
                .admit_error(identity.clone(), RelayErrorTunnelSender::new(rejected_tx),)
                .expect_err("error without control must fail"),
            "Relay error tunnel requires an active control tunnel"
        );
        assert!(registry.error_sender(&identity).is_none());

        registry
            .attach_control(identity.clone())
            .expect("control tunnel");
        let (accepted_tx, _accepted_rx) = mpsc::channel(1);
        registry
            .admit_error(identity.clone(), RelayErrorTunnelSender::new(accepted_tx))
            .expect("error with control");
        assert!(registry.error_sender(&identity).is_some());
    }
}
