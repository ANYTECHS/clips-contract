use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    TokenNotFound = 4,
    AlreadyMinted = 5,
    InvalidBasisPoints = 6,
    InvalidRecipient = 7,
    Paused = 8,
    NotPaused = 9,
    MintCooldown = 10,
    /// Returned when an attempt is made to mint a clip that has already been
    /// minted. Each `clip_id` may only be minted once; subsequent mint calls
    /// for the same `clip_id` must return this error.
    ClipAlreadyMinted = 11,
}
