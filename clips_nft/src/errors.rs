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
}
