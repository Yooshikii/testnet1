use crate::{
    errors::{IndexError, IndexResult},
    IDENT,
};
use async_trait::async_trait;
use vecno_consensus_notify::{notification as consensus_notification, notification::Notification as ConsensusNotification};
use vecno_core::{debug, trace};
use vecno_index_core::notification::{Notification, PruningPointUtxoSetOverrideNotification, UtxosChangedNotification};
use vecno_notify::{
    collector::{Collector, CollectorNotificationReceiver},
    error::Result,
    events::EventType,
    notification::Notification as NotificationTrait,
    notifier::DynNotify,
};
use vecno_utils::triggers::SingleTrigger;
use vecno_utxoindex::api::UtxoIndexProxy;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Processor processes incoming consensus UtxosChanged and PruningPointUtxoSetOverride
/// notifications submitting them to a UtxoIndex.
///
/// It also acts as a [`Collector`], converting the incoming consensus notifications
/// into their pending local versions and relaying them to a local notifier.
#[derive(Debug)]
pub struct Processor {
    /// An optional UTXO indexer
    utxoindex: Option<UtxoIndexProxy>,

    recv_channel: CollectorNotificationReceiver<ConsensusNotification>,

    /// Has this collector been started?
    is_started: Arc<AtomicBool>,

    collect_shutdown: Arc<SingleTrigger>,
}

impl Processor {
    pub fn new(utxoindex: Option<UtxoIndexProxy>, recv_channel: CollectorNotificationReceiver<ConsensusNotification>) -> Self {
        Self {
            utxoindex,
            recv_channel,
            collect_shutdown: Arc::new(SingleTrigger::new()),
            is_started: Arc::new(AtomicBool::new(false)),
        }
    }

    fn spawn_collecting_task(self: Arc<Self>, notifier: DynNotify<Notification>) {
        // The task can only be spawned once
        if self.is_started.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return;
        }
        tokio::spawn(async move {
            trace!("[Index processor] collecting task starting");

            while let Ok(notification) = self.recv_channel.recv().await {
                match self.process_notification(notification).await {
                    Ok(notification) => match notifier.notify(notification) {
                        Ok(_) => (),
                        Err(err) => {
                            trace!("[Index processor] notification sender error: {err:?}");
                        }
                    },
                    Err(err) => {
                        trace!("[Index processor] error while processing a consensus notification: {err:?}");
                    }
                }
            }

            debug!("[Index processor] notification stream ended");
            self.collect_shutdown.trigger.trigger();
            trace!("[Index processor] collecting task ended");
        });
    }

    async fn process_notification(self: &Arc<Self>, notification: ConsensusNotification) -> IndexResult<Notification> {
        match notification {
            ConsensusNotification::UtxosChanged(utxos_changed) => {
                Ok(Notification::UtxosChanged(self.process_utxos_changed(utxos_changed).await?))
            }
            ConsensusNotification::PruningPointUtxoSetOverride(_) => {
                Ok(Notification::PruningPointUtxoSetOverride(PruningPointUtxoSetOverrideNotification {}))
            }
            _ => Err(IndexError::NotSupported(notification.event_type())),
        }
    }

    async fn process_utxos_changed(
        self: &Arc<Self>,
        notification: consensus_notification::UtxosChangedNotification,
    ) -> IndexResult<UtxosChangedNotification> {
        trace!("[{IDENT}]: processing {:?}", notification);
        if let Some(utxoindex) = self.utxoindex.clone() {
            let converted_notification: UtxosChangedNotification =
                utxoindex.update(notification.accumulated_utxo_diff.clone(), notification.virtual_parents).await?.into();
            debug!(
                "IDXPRC, Creating UtxosChanged notifications with {} added and {} removed utxos",
                converted_notification.added.len(),
                converted_notification.removed.len()
            );
            return Ok(converted_notification);
        };
        Err(IndexError::NotSupported(EventType::UtxosChanged))
    }

    async fn join_collecting_task(&self) -> Result<()> {
        trace!("[Index processor] joining");
        self.collect_shutdown.listener.clone().await;
        debug!("[Index processor] terminated");
        Ok(())
    }
}

#[async_trait]
impl Collector<Notification> for Processor {
    fn start(self: Arc<Self>, notifier: DynNotify<Notification>) {
        self.spawn_collecting_task(notifier);
    }

    async fn join(self: Arc<Self>) -> Result<()> {
        self.join_collecting_task().await
    }
}