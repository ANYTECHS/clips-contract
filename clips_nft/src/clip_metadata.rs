use soroban_sdk::{contracttype, Address, String};

/// Primary metadata structure for a ClipCash NFT.
///
/// Stores all metadata associated with a clip before minting.
#[contracttype]
#[derive(Clone)]
pub struct ClipMetadata {
    /// Human-readable title of the clip.
    pub title: String,
    /// Short description of the clip.
    pub description: String,
    /// Thumbnail image URI (IPFS or Arweave).
    pub thumbnail: String,
    /// Full content URI pointing to the clip on IPFS or Arweave.
    pub ipfs_uri: String,
    /// Address of the clip creator.
    pub creator: Address,
    /// Unix timestamp (seconds) when the clip was created.
    pub created_at: u64,
}
