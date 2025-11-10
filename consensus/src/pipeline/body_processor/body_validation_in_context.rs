use super::BlockBodyProcessor;
use crate::{
    errors::{BlockProcessResult, RuleError},
    model::stores::{ghostdag::GhostdagStoreReader, headers::HeaderStoreReader, statuses::StatusesStoreReader},
    processes::{
        transaction_validator::{
            tx_validation_in_header_context::{LockTimeArg, LockTimeType},
            TransactionValidator,
        },
        window::WindowManager,
    },
};
use vecno_consensus_core::{block::Block, errors::tx::TxRuleError};
use vecno_database::prelude::StoreResultExtensions;
use vecno_hashes::Hash;
use once_cell::unsync::Lazy;
use std::sync::Arc;

impl BlockBodyProcessor {
    pub fn validate_body_in_context(self: &Arc<Self>, block: &Block) -> BlockProcessResult<()> {
        self.check_parent_bodies_exist(block)?;
        self.check_coinbase_outputs_limit(block)?;
        self.check_coinbase_blue_score_and_subsidy(block)?;
        self.check_block_transactions_in_context(block)
    }

    fn check_block_transactions_in_context(self: &Arc<Self>, block: &Block) -> BlockProcessResult<()> {
        // Use lazy evaluation to avoid unnecessary work, as most of the time we expect the txs not to have lock time.
        let lazy_pmt_res = Lazy::new(|| self.window_manager.calc_past_median_time_for_known_hash(block.hash()));

        for tx in block.transactions.iter() {
            let lock_time_arg = match TransactionValidator::get_lock_time_type(tx) {
                LockTimeType::Finalized => LockTimeArg::Finalized,
                LockTimeType::DaaScore => LockTimeArg::DaaScore(block.header.daa_score),
                // We only evaluate the pmt calculation when actually needed
                LockTimeType::Time => LockTimeArg::MedianTime((*lazy_pmt_res).clone()?),
            };
            if let Err(e) = self.transaction_validator.validate_tx_in_header_context(tx, block.header.daa_score, lock_time_arg) {
                return Err(RuleError::TxInContextFailed(tx.id(), e));
            };
        }
        Ok(())
    }

    fn check_parent_bodies_exist(self: &Arc<Self>, block: &Block) -> BlockProcessResult<()> {
        let statuses_read_guard = self.statuses_store.read();
        let missing: Vec<Hash> = block
            .header
            .direct_parents()
            .iter()
            .copied()
            .filter(|parent| {
                let status_option = statuses_read_guard.get(*parent).unwrap_option();
                status_option.is_none_or(|s| !s.has_block_body())
            })
            .collect();
        if !missing.is_empty() {
            return Err(RuleError::MissingParents(missing));
        }

        Ok(())
    }

    fn check_coinbase_outputs_limit(&self, block: &Block) -> BlockProcessResult<()> {
        // [Starlight]: coinbase_outputs_limit depends on ghostdag k and thus depends on fork activation
        // which makes it header contextual.
        //
        // TODO (post HF): move this check back to transaction in isolation validation

        // [Starlight]: Ghostdag k activation is decided based on selected parent DAA score
        // so we follow the same methodology for coinbase output limit (which is driven from the
        // actual bound on the number of blue blocks in the mergeset).
        //
        // Note that body validation in context is not called for trusted blocks, so we can safely assume
        // the selected parent exists and its daa score is accessible
        let selected_parent = self.ghostdag_store.get_selected_parent(block.hash()).unwrap();
        let selected_parent_daa_score = self.headers_store.get_daa_score(selected_parent).unwrap();
        let coinbase_outputs_limit = self.ghostdag_k.get(selected_parent_daa_score) as u64 + 2;

        let tx = &block.transactions[0];
        if tx.outputs.len() as u64 > coinbase_outputs_limit {
            return Err(RuleError::TxInIsolationValidationFailed(
                tx.id(),
                TxRuleError::CoinbaseTooManyOutputs(tx.outputs.len(), coinbase_outputs_limit),
            ));
        }
        Ok(())
    }

    fn check_coinbase_blue_score_and_subsidy(self: &Arc<Self>, block: &Block) -> BlockProcessResult<()> {
        match self.coinbase_manager.deserialize_coinbase_payload(&block.transactions[0].payload) {
            Ok(data) => {
                if data.blue_score != block.header.blue_score {
                    return Err(RuleError::BadCoinbasePayloadBlueScore(data.blue_score, block.header.blue_score));
                }

                let expected_subsidy = self.coinbase_manager.calc_block_subsidy(block.header.daa_score);

                if data.subsidy != expected_subsidy {
                    return Err(RuleError::WrongSubsidy(expected_subsidy, data.subsidy));
                }

                Ok(())
            }
            Err(e) => Err(RuleError::BadCoinbasePayload(e)),
        }
    }
}