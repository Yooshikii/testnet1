use crate::{
    api::UtxoIndexApi,
    errors::{UtxoIndexError, UtxoIndexResult},
    model::{CirculatingSupply, UtxoChanges, UtxoSetByScriptPublicKey},
    stores::store_manager::Store,
    update_container::UtxoIndexChanges,
    IDENT,
};
use kaspa_consensus_core::{tx::ScriptPublicKeys, utxo::utxo_diff::UtxoDiff, BlockHashSet};
use kaspa_consensusmanager::{ConsensusManager, ConsensusResetHandler};
use kaspa_core::{info, trace};
use kaspa_database::prelude::{StoreError, StoreResult, DB};
use kaspa_hashes::Hash;
use kaspa_index_core::indexed_utxos::BalanceByScriptPublicKey;
use kaspa_utils::arc::ArcExtensions;
use parking_lot::RwLock;
use std::{
    fmt::Debug,
    sync::{Arc, Weak},
};

const RESYNC_CHUNK_SIZE: usize = 2048; //Increased from 1k (used in go-kaspad), for quicker resets, while still having a low memory footprint.

/// UtxoIndex indexes `CompactUtxoEntryCollections` by [`ScriptPublicKey`](kaspa_consensus_core::tx::ScriptPublicKey),
/// commits them to its owns store, and emits changes.
/// Note: The UtxoIndex struct by itself is not thread save, only correct usage of the supplied RwLock via `new` makes it so.
/// please follow guidelines found in the comments under `utxoindex::core::api::UtxoIndexApi` for proper thread safety.
pub struct UtxoIndex {
    consensus_manager: Arc<ConsensusManager>,
    store: Store,
    /// A runtime value holding a monotonic supply value. Used to prevent supply fluctuations due
    /// to the single round gap between fee deduction and its payment to miners
    monotonic_circulating_supply: CirculatingSupply,
}

impl UtxoIndex {
    /// Creates a new [`UtxoIndex`] within a [`RwLock`]
    pub fn new(consensus_manager: Arc<ConsensusManager>, db: Arc<DB>) -> UtxoIndexResult<Arc<RwLock<Self>>> {
        let mut utxoindex =
            Self { consensus_manager: consensus_manager.clone(), store: Store::new(db), monotonic_circulating_supply: 0 };
        if !utxoindex.is_synced()? {
            utxoindex.resync()?;
        } else {
            utxoindex.monotonic_circulating_supply = utxoindex.store.get_circulating_supply()?;
        }
        let utxoindex = Arc::new(RwLock::new(utxoindex));
        consensus_manager.register_consensus_reset_handler(Arc::new(UtxoIndexConsensusResetHandler::new(Arc::downgrade(&utxoindex))));
        Ok(utxoindex)
    }
}

impl UtxoIndexApi for UtxoIndex {
    /// Retrieve circulating supply from the utxoindex db.
    fn get_circulating_supply(&self) -> StoreResult<u64> {
        trace!("[{0}] retrieving circulating supply", IDENT);

        Ok(self.monotonic_circulating_supply)
    }

    /// Retrieve utxos by script public keys from the utxoindex db.
    fn get_utxos_by_script_public_keys(&self, script_public_keys: ScriptPublicKeys) -> StoreResult<UtxoSetByScriptPublicKey> {
        trace!("[{0}] retrieving utxos from {1} script public keys", IDENT, script_public_keys.len());

        self.store.get_utxos_by_script_public_key(script_public_keys)
    }

    /// Retrieve utxos by script public keys from the utxoindex db.
    fn get_balance_by_script_public_keys(&self, script_public_keys: ScriptPublicKeys) -> StoreResult<BalanceByScriptPublicKey> {
        trace!("[{0}] retrieving utxos from {1} script public keys", IDENT, script_public_keys.len());

        self.store.get_balance_by_script_public_key(script_public_keys)
    }

    /// Retrieve the stored tips of the utxoindex.
    fn get_utxo_index_tips(&self) -> StoreResult<Arc<BlockHashSet>> {
        trace!("[{0}] retrieving tips", IDENT);

        self.store.get_tips()
    }

    /// Updates the [UtxoIndex] via the virtual state supplied:
    /// 1) Saves updated utxo differences, virtual parent hashes and circulating supply to the database.
    /// 2) returns an event about utxoindex changes.
    fn update(&mut self, utxo_diff: Arc<UtxoDiff>, tips: Arc<Vec<Hash>>) -> UtxoIndexResult<UtxoChanges> {
        trace!("[{0}] updating...", IDENT);
        trace!("[{0}] adding {1} utxos", IDENT, utxo_diff.add.len());
        trace!("[{0}] removing {1} utxos", IDENT, utxo_diff.remove.len());

        // Initiate update container
        let mut utxoindex_changes = UtxoIndexChanges::new();
        utxoindex_changes.update_utxo_diff(utxo_diff.unwrap_or_clone());
        utxoindex_changes.set_tips(tips.unwrap_or_clone().to_vec());

        // Commit changed utxo state to db
        self.store.update_utxo_state(&utxoindex_changes.utxo_changes.added, &utxoindex_changes.utxo_changes.removed, false)?;

        // Update the stored circulating supply with the accumulated delta of the changes
        let updated_circulating_supply = self.store.update_circulating_supply(utxoindex_changes.supply_change, false)?;

        // Update the monotonic runtime value
        if updated_circulating_supply > self.monotonic_circulating_supply {
            self.monotonic_circulating_supply = updated_circulating_supply;
        }

        // Commit new consensus virtual tips.
        self.store.set_tips(utxoindex_changes.tips, false)?; //we expect new tips with every virtual!

        // Return the resulting changes in utxoindex.
        Ok(utxoindex_changes.utxo_changes)
    }

    /// Checks to see if the [UtxoIndex] is sync'd. This is done via comparing the utxoindex committed `VirtualParent` hashes with those of the consensus database.
    ///
    /// **Note:** Due to sync gaps between the utxoindex and consensus, this function is only reliable while consensus is not processing new blocks.
    fn is_synced(&self) -> UtxoIndexResult<bool> {
        trace!("[{0}] checking sync status...", IDENT);

        let consensus = self.consensus_manager.consensus();
        let session = futures::executor::block_on(consensus.session_blocking());

        let utxoindex_tips = self.store.get_tips();
        match utxoindex_tips {
            Ok(utxoindex_tips) => {
                let consensus_tips = session.get_virtual_parents();
                let res = *utxoindex_tips == consensus_tips;
                trace!("[{0}] sync status is {1}", IDENT, res);
                Ok(res)
            }
            Err(error) => match error {
                StoreError::KeyNotFound(_) => {
                    //Means utxoindex tips database is empty i.e. not sync'd.
                    trace!("[{0}] sync status is {1}", IDENT, false);
                    Ok(false)
                }
                other_store_errors => Err(UtxoIndexError::StoreAccessError(other_store_errors)),
            },
        }
    }
    /// Deletes and reinstates the utxoindex database, syncing it from scratch via the consensus database.
    ///
    /// **Notes:**
    /// 1) There is an implicit expectation that the consensus store must have VirtualParent tips. i.e. consensus database must be initiated.
    /// 2) resyncing while consensus notifies of utxo differences, may result in a corrupted db.
    fn resync(&mut self) -> UtxoIndexResult<()> {
        info!("Resyncing the utxoindex...");

        self.store.delete_all()?;
        let consensus = self.consensus_manager.consensus();
        let session = futures::executor::block_on(consensus.session_blocking());

        let consensus_tips = session.get_virtual_parents();
        let mut circulating_supply: CirculatingSupply = 0;

        //Initial batch is without specified seek and none-skipping.
        let mut virtual_utxo_batch = session.get_virtual_utxos(None, RESYNC_CHUNK_SIZE, false);
        let mut current_chunk_size = virtual_utxo_batch.len();
        trace!("[{0}] resyncing with batch of {1} utxos from consensus db", IDENT, current_chunk_size);
        // While loop stops resync attempts from an empty utxo db, and unneeded processing when the utxo state size happens to be a multiple of [`RESYNC_CHUNK_SIZE`]
        while current_chunk_size > 0 {
            // Potential optimization TODO: iterating virtual utxos into an [UtxoIndexChanges] struct is a bit of overhead (i.e. a potentially unneeded loop),
            // but some form of pre-iteration is done to extract and commit circulating supply separately.

            let mut utxoindex_changes = UtxoIndexChanges::new(); //reset changes.

            let next_outpoint_from = Some(virtual_utxo_batch.last().expect("expected a last outpoint").0);
            utxoindex_changes.add_utxos_from_vector(virtual_utxo_batch);

            circulating_supply += utxoindex_changes.supply_change as CirculatingSupply;

            self.store.update_utxo_state(&utxoindex_changes.utxo_changes.added, &utxoindex_changes.utxo_changes.removed, true)?;

            if current_chunk_size < RESYNC_CHUNK_SIZE {
                break;
            };

            virtual_utxo_batch = session.get_virtual_utxos(next_outpoint_from, RESYNC_CHUNK_SIZE, true);
            current_chunk_size = virtual_utxo_batch.len();
            trace!("[{0}] resyncing with batch of {1} utxos from consensus db", IDENT, current_chunk_size);
        }

        // Commit to the remaining stores.

        trace!("[{0}] committing circulating supply {1} from consensus db", IDENT, circulating_supply);
        self.store.insert_circulating_supply(circulating_supply, true)?;
        self.monotonic_circulating_supply = circulating_supply;

        trace!("[{0}] committing consensus tips {consensus_tips:?} from consensus db", IDENT);
        self.store.set_tips(consensus_tips, true)?;

        Ok(())
    }

    // This can have a big memory footprint, so it should be used only for tests.
    fn get_all_outpoints(&self) -> StoreResult<std::collections::HashSet<kaspa_consensus_core::tx::TransactionOutpoint>> {
        self.store.get_all_outpoints()
    }
}

impl Debug for UtxoIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UtxoIndex").finish()
    }
}

struct UtxoIndexConsensusResetHandler {
    utxoindex: Weak<RwLock<UtxoIndex>>,
}

impl UtxoIndexConsensusResetHandler {
    fn new(utxoindex: Weak<RwLock<UtxoIndex>>) -> Self {
        Self { utxoindex }
    }
}

impl ConsensusResetHandler for UtxoIndexConsensusResetHandler {
    fn handle_consensus_reset(&self) {
        if let Some(utxoindex) = self.utxoindex.upgrade() {
            utxoindex.write().resync().unwrap();
        }
    }
}