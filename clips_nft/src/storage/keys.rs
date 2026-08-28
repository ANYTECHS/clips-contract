use soroban_sdk::{contracttype, Address, BytesN, String};

/// All persistent/instance storage keys for the contract.
///
/// Compact enum variants keep key sizes minimal on-chain.
/// Centralizes all storage keys used throughout the contract to prevent duplicate keys.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    // ── instance ───────────────────────────────────────────
    /// Global contract administrator/owner address.
    Admin,
    /// Auto-increment counter for the next token ID.
    NextTokenId,
    /// Monotonically increasing batch identifier bumped on every batch mint.
    NextBatchId,
    /// Total number of active tokens in circulation.
    TotalSupply,
    /// Pause state of the contract (boolean flag).
    Paused,
    /// Authorized backend signature public key (Ed25519 32-byte array).
    Signer,
    /// Global contract configuration parameters (fees, limits, cool downs).
    Config,
    /// List of supported payment currencies.
    SupportedCurrencies,
    /// Contract platform fee configuration.
    PlatformFee,
    /// Recipient address for platform fees.
    PlatformRecipient,
    /// Default royalty basis points (applied when no per-token override is set).
    DefaultRoyaltyBps,
    /// Total platform revenue accumulated (in smallest unit).
    PlatformRevenue,
    /// Configurable maximum allowed size in bytes for a metadata URI.
    MaxMetadataSize,
    /// Global metadata schema version used for migrations.
    MetadataVersion,
    /// Total count of tokens minted across all time (independent counter).
    TokenCounter,
    /// Persisted contract version record (version + upgrade timestamp).
    ContractVersion,
    /// Human-readable name of the collection (e.g. "ClipCash Clips").
    Name,
    /// Ticker symbol of the collection (e.g. "CLIP").
    Symbol,
    /// Structured collection-level metadata blob.
    CollectionMetadata,
    /// Marketplace listing counter.
    ListingCounter,
    /// Global marketplace configuration.
    MarketplaceConfig,

    // ── persistent ─────────────────────────────────────────
    /// Marketplace listing keyed by listing ID.
    Listings(u32),
    /// Marketplace offers keyed by listing ID.
    Offers(u32),
    /// Sale history keyed by token ID.
    SaleHistory(u32),
    /// Owner + clip_id data for a token.
    Token(u32),
    /// Canonical metadata URI linked to a token.
    Metadata(u32),
    /// Royalty configurations for a token.
    Royalty(u32),
    /// Maps clip_id to token_id to prevent double-minting.
    ClipIdMinted(u32),
    /// Blacklisted status for an address.
    Blacklisted(Address),
    /// Single-token approval: address approved to transfer token_id.
    Approval(u32),
    /// Operator approval mapping: (owner, operator) to approval status.
    OperatorApproval(Address, Address),
    /// Total supply count of a specific collection.
    CollectionSupply(u32),
    /// Maps token_id to clip_id (reverse mapping of ClipIdMinted).
    TokenClipId(u32),
    /// Existence marker for clip_id (boolean flag).
    ClipMinted(u32),
    /// AI-generated virality score for a token.
    ViralityScore(u32),
    /// Originating social platform indicator for a token.
    SocialPlatform(u32),
    /// Original video source identifier.
    VideoSourceId(u32),
    /// Original video source web URL.
    VideoSourceUrl(u32),
    /// Uniqueness check index mapping metadata URI string to token ID.
    MetadataIndex(String),
    /// Approved minter address (single minter role).
    ApprovedMinter(Address),
    /// Ordered list of token IDs owned by a wallet address.
    WalletTokens(Address),
    /// Original creator address for a token.
    Creator(u32),
    /// Soulbound / frozen status of a token (marks it non-transferable).
    FrozenToken(u32),
    /// Historical logs of royalty payments for a token.
    RoyaltyHistory(u32),
    /// Direct address of the royalty recipient.
    RoyaltyRecipient(u32),
    /// Metadata URI stored separately from the core token record.
    TokenUri(u32),
    /// Structured details about a clip (clip title, description, etc.).
    ClipInfo(u32),
    /// Timestamps for metadata updates (creation + last update epoch seconds).
    MetadataTimestamps(u32),
    /// Flag indicating that the per-token metadata has been updated at least once.
    MetadataUpdated(u32),
    /// Marks a backend signature hash as consumed to prevent replay.
    UsedSignature(BytesN<32>),
    /// Address transaction nonce for signature replay prevention.
    Nonce(Address),
    /// Media thumbnail URI associated with a minted NFT.
    ThumbnailUri(u32),
    /// Media preview video URI associated with a minted NFT.
    PreviewVideoUri(u32),
    /// Legacy thumbnail key alias.
    Thumbnail(u32),
    /// Legacy preview key alias.
    PreviewUri(u32),
    /// Per-token royalty percentage in basis points.
    RoyaltyPercentage(u32),
    /// Portfolio index of creator's tokens.
    CreatorTokens(Address),
    /// Portfolio index of owner's tokens.
    OwnerTokens(Address),
    /// Collection ID linked to a token.
    TokenCollection(u32),
    /// Registered status of a collection.
    CollectionRegistered(u32),
    /// List of tokens belonging to a collection.
    CollectionMembers(u32),
    /// Registered metadata record presence key (URI -> boolean).
    MetadataRecord(String),
    /// Direct token owner address record.
    TokenOwner(u32),
    /// Administrator accounts status.
    Administrator(Address),
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_storage_key_variants_distinguishability() {
        // Simple test to ensure variants compile and are distinct
        let key_admin = StorageKey::Admin;
        let key_paused = StorageKey::Paused;
        assert_ne!(key_admin, key_paused);
    }
}
