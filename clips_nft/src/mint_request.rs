use soroban_sdk::{contracttype, Address, String};

use crate::Royalty;

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
    /// Metadata URI (IPFS or Arweave) for the NFT.
    pub metadata_uri: String,
    /// Royalty configuration for secondary sales.
    pub royalty_info: Royalty,
}
