use soroban_sdk::{contracterror, symbol_short, Symbol};

/// Error type for transfers attempted when the NFT is in an incompatible state.
///
/// Each variant carries a stable numeric code in the 280–284 transfer-state
/// range so that off-chain consumers can rely on them across releases.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InvalidTransferState {
    /// The NFT has been burned and can no longer be transferred.
    Burned = 280,
    /// The NFT is frozen and transfers are currently disallowed.
    Frozen = 281,
    /// The NFT has no recorded owner.
    MissingOwner = 282,
    /// The recorded ownership state is inconsistent or invalid.
    InvalidOwnership = 283,
    /// The token is inactive and cannot be transferred.
    InactiveToken = 284,
}

impl InvalidTransferState {
    /// Returns the stable numeric code associated with this error.
    pub fn code(&self) -> u32 {
        match self {
            InvalidTransferState::Burned => 280,
            InvalidTransferState::Frozen => 281,
            InvalidTransferState::MissingOwner => 282,
            InvalidTransferState::InvalidOwnership => 283,
            InvalidTransferState::InactiveToken => 284,
        }
    }

    /// Returns a short, stable symbol name for this error.
    pub fn name(&self) -> Symbol {
        match self {
            InvalidTransferState::Burned => symbol_short!("Burned"),
            InvalidTransferState::Frozen => symbol_short!("Frozen"),
            InvalidTransferState::MissingOwner => symbol_short!("Missing"),
            InvalidTransferState::InvalidOwnership => symbol_short!("InvOwner"),
            InvalidTransferState::InactiveToken => symbol_short!("Inactive"),
        }
    }

    /// Reconstructs the error from its stable numeric code.
    ///
    /// Returns `None` when the code does not map to a known variant.
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            280 => Some(InvalidTransferState::Burned),
            281 => Some(InvalidTransferState::Frozen),
            282 => Some(InvalidTransferState::MissingOwner),
            283 => Some(InvalidTransferState::InvalidOwnership),
            284 => Some(InvalidTransferState::InactiveToken),
            _ => None,
        }
    }
}

/// Pure guard that validates a transfer state reason.
///
/// Returns `Ok(())` when the reason is a recognized transfer-state error and
/// echoes it back as `Err(reason)` otherwise. This helper performs no storage
/// access and is safe to call from any context.
pub fn ensure_transfer_state(reason: InvalidTransferState) -> Result<(), InvalidTransferState> {
    match InvalidTransferState::from_code(reason.code()) {
        Some(known) if known == reason => Ok(()),
        _ => Err(reason),
    }
}

/// Constructs the `Burned` transfer-state error.
pub fn burned() -> InvalidTransferState {
    InvalidTransferState::Burned
}

/// Constructs the `Frozen` transfer-state error.
pub fn frozen() -> InvalidTransferState {
    InvalidTransferState::Frozen
}

/// Constructs the `MissingOwner` transfer-state error.
pub fn missing_owner() -> InvalidTransferState {
    InvalidTransferState::MissingOwner
}

/// Constructs the `InvalidOwnership` transfer-state error.
pub fn invalid_ownership() -> InvalidTransferState {
    InvalidTransferState::InvalidOwnership
}

/// Constructs the `InactiveToken` transfer-state error.
pub fn inactive_token() -> InvalidTransferState {
    InvalidTransferState::InactiveToken
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [InvalidTransferState; 5] = [
        InvalidTransferState::Burned,
        InvalidTransferState::Frozen,
        InvalidTransferState::MissingOwner,
        InvalidTransferState::InvalidOwnership,
        InvalidTransferState::InactiveToken,
    ];

    #[test]
    fn codes_are_stable_and_in_range() {
        assert_eq!(InvalidTransferState::Burned.code(), 280);
        assert_eq!(InvalidTransferState::Frozen.code(), 281);
        assert_eq!(InvalidTransferState::MissingOwner.code(), 282);
        assert_eq!(InvalidTransferState::InvalidOwnership.code(), 283);
        assert_eq!(InvalidTransferState::InactiveToken.code(), 284);

        for reason in ALL.iter() {
            let code = reason.code();
            assert!((280..=284).contains(&code));
        }
    }

    #[test]
    fn names_are_distinct() {
        let mut names: [Symbol; 5] = [
            symbol_short!("Burned"),
            symbol_short!("Frozen"),
            symbol_short!("Missing"),
            symbol_short!("InvOwner"),
            symbol_short!("Inactive"),
        ];
        for (i, reason) in ALL.iter().enumerate() {
            assert_eq!(reason.name(), names[i]);
        }
        // Ensure all names are unique.
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                assert_ne!(names[i], names[j]);
            }
        }
        // Silence unused-mut warning in case of future edits.
        names[0] = symbol_short!("Burned");
    }

    #[test]
    fn from_code_round_trips() {
        for reason in ALL.iter() {
            let code = reason.code();
            assert_eq!(InvalidTransferState::from_code(code), Some(*reason));
        }
    }

    #[test]
    fn from_code_rejects_unknown() {
        assert_eq!(InvalidTransferState::from_code(0), None);
        assert_eq!(InvalidTransferState::from_code(279), None);
        assert_eq!(InvalidTransferState::from_code(285), None);
        assert_eq!(InvalidTransferState::from_code(u32::MAX), None);
    }

    #[test]
    fn ensure_transfer_state_accepts_known() {
        for reason in ALL.iter() {
            assert_eq!(ensure_transfer_state(*reason), Ok(()));
        }
    }

    #[test]
    fn constructors_match_variants() {
        assert_eq!(burned(), InvalidTransferState::Burned);
        assert_eq!(frozen(), InvalidTransferState::Frozen);
        assert_eq!(missing_owner(), InvalidTransferState::MissingOwner);
        assert_eq!(invalid_ownership(), InvalidTransferState::InvalidOwnership);
        assert_eq!(inactive_token(), InvalidTransferState::InactiveToken);
    }
}