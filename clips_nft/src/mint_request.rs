use soroban_sdk::{contracttype, Address, String, Vec};

use crate::{config::Config, storage_constants::MIN_BATCH_MINT_SIZE, types::Error, Royalty};

/// Internal DTO used by the contract when preparing an NFT mint request.
///
/// Aggregates all required data before a mint transaction is executed.
#[contracttype]
#[derive(Clone)]
pub struct MintRequest {
    /// Off-chain clip identifier.
    pub clip_id: u32,
    /// Address that will own the minted NFT.
    pub owner: Address,
    /// Original creator of the clip — persisted for attribution and royalty
    /// distribution (issue #665).
    pub creator: Address,
    /// Metadata URI (IPFS or Arweave) for the NFT.
    pub metadata_uri: String,
    /// Optional thumbnail image URI for marketplace display (issue #668).
    pub thumbnail_uri: Option<String>,
    /// Optional preview video URI for marketplace previews (issue #669).
    pub preview_video_uri: Option<String>,
    /// Royalty configuration for secondary sales.
    pub royalty_info: Royalty,
    /// Optional creator address. If None, defaults to the owner address.
    pub creator_address: Option<Address>,
    /// Optional human-readable display name for the creator.
    pub creator_display_name: Option<String>,
}

/// Request structure for batch NFT minting.
///
/// Encapsulates multiple mint requests to be processed in a single transaction.
#[contracttype]
#[derive(Clone)]
pub struct BatchMintRequest {
    /// List of individual mint requests.
    pub requests: Vec<MintRequest>,
}

impl BatchMintRequest {
    /// Validates the batch size against the configured max_batch_mint_size.
    ///
    /// # Arguments
    /// * `config` - The global contract config
    ///
    /// # Errors
    /// Returns `Error::InvalidConfig` if the batch size is less than 1,
    /// or `Error::BatchLimitExceeded` if request size exceeds `config.max_batch_mint_size`.
    pub fn validate_batch_size(&self, config: &Config) -> Result<(), Error> {
        let len = self.requests.len() as u32;
        if len < MIN_BATCH_MINT_SIZE {
            return Err(Error::InvalidConfig);
        }
        if len > config.max_batch_mint_size {
            return Err(Error::BatchLimitExceeded);
        }
        Ok(())
    }

    /// Read the configured batch limit from contract storage (or storage default) and validate request size.
    ///
    /// # Errors
    /// Returns `Error::InvalidConfig` if batch size is 0,
    /// or `Error::BatchLimitExceeded` if request size exceeds the configured batch limit.
    pub fn validate_against_env(&self, env: &soroban_sdk::Env) -> Result<(), Error> {
        let len = self.requests.len() as u32;
        if len < MIN_BATCH_MINT_SIZE {
            return Err(Error::InvalidConfig);
        }
        let max_limit = crate::config::get_config(env)
            .map(|c| c.max_batch_mint_size)
            .unwrap_or(crate::storage_constants::MAX_BATCH_MINT_SIZE);

        if len > max_limit {
            return Err(Error::BatchLimitExceeded);
        }
        Ok(())
    }
}
