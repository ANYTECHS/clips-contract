use soroban_sdk::contracttype;

/// All persistent/instance storage keys for the contract.
///
/// Compact enum variants keep key sizes minimal on-chain.
#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
    // ── instance ───────────────────────────────────────────
    /// Global [`Config`] (admin, fees, limits).
    Config,
    /// Auto-increment counter for the next token ID.
    NextTokenId,
    /// Pause state (`bool`).
    Paused,
    /// Backend Ed25519 public key (`BytesN<32>`).
    Signer,

    // ── persistent ─────────────────────────────────────────
    /// Owner + clip_id for a token.
    Token(u32),
    /// Metadata URI for a token.
    Metadata(u32),
    /// Royalty config for a token.
    Royalty(u32),
    /// Maps clip_id → token_id to prevent double-minting.
    ClipIdMinted(u32),
}
