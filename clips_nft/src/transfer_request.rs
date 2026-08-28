//! Transfer request structures for single and batch NFT transfers.
//!
//! This module defines the data structures used to describe NFT transfer
//! operations before they are validated and executed. It mirrors the design
//! of [`crate::mint_request`]: a lightweight DTO layer that carries all
//! required inputs without owning any storage or business logic.
//!
//! # Structures
//!
//! | Struct | Purpose |
//! |---|---|
//! | [`TransferRequest`] | Describes moving a single NFT from one owner to another. |
//! | [`BatchTransferRequest`] | Wraps multiple [`TransferRequest`]s for atomic batch processing. |
//!
//! # Serialization
//!
//! Both structs are annotated with `#[contracttype]`, which instructs the
//! Soroban SDK to generate XDR-compatible serialization/deserialization code.
//! This means they can be:
//! - Passed as contract function arguments or return values.
//! - Stored in contract persistent or instance storage.
//! - Decoded by off-chain clients using the standard XDR format.
//!
//! # Example
//!
//! ```rust,ignore
//! use soroban_sdk::{Address, Env, Vec};
//! use clips_nft::transfer_request::{BatchTransferRequest, TransferRequest};
//!
//! let env = Env::default();
//! let from      = Address::generate(&env);
//! let to        = Address::generate(&env);
//! let timestamp = env.ledger().timestamp();
//!
//! let mut requests = Vec::new(&env);
//! requests.push_back(TransferRequest {
//!     from: from.clone(), to: to.clone(), token_id: 1, timestamp, memo: None,
//! });
//! requests.push_back(TransferRequest {
//!     from: from.clone(), to: to.clone(), token_id: 2, timestamp, memo: None,
//! });
//!
//! let batch = BatchTransferRequest { requests };
//! assert!(batch.validate_batch_size(50).is_ok());
//! ```

use soroban_sdk::{contracttype, Address, Env, String, Vec};

use crate::storage_constants::{MAX_BATCH_TRANSFER_SIZE, MIN_BATCH_TRANSFER_SIZE};
use crate::types::Error;
use crate::types::TokenId;

// ─── Single transfer ──────────────────────────────────────────────────────────

/// Describes the transfer of a single NFT from one address to another.
///
/// `TransferRequest` is the primary struct used for NFT ownership transfers
/// in the ClipCash contract. It is a pure data container — it does **not**
/// perform authorization or state writes. Those responsibilities belong to
/// the transfer execution layer.
///
/// # Fields
///
/// - `token_id`  — On-chain identifier of the token being transferred.
/// - `from`      — Sender: the current owner of the token. Must hold
///                 ownership at execution time; the executor asserts this.
/// - `to`        — Recipient: the destination address that will receive
///                 ownership.
/// - `timestamp` — Ledger timestamp (seconds since Unix epoch) recorded
///                 when the request is constructed. Allows indexers and
///                 auditors to establish a chronological transfer history
///                 without additional storage reads.
/// - `memo`      — Optional human-readable note (e.g. a gift message or
///                 internal reference ID). Carried in the request for
///                 off-chain indexers; not persisted in contract storage.
///
/// # Serialization
///
/// The struct derives `#[contracttype]`, so the Soroban SDK generates full
/// XDR serialization/deserialization automatically. It can be passed as a
/// contract argument, returned from a function, or stored under any
/// `DataKey` variant in persistent or instance storage.
///
/// # Validation notes
///
/// - `from` and `to` must be distinct addresses; the executor rejects
///   self-transfers.
/// - The token must exist and must not be frozen (soulbound) at execution
///   time.
/// - Authorization that `from` is the actual caller (or an approved
///   operator) is enforced by the transfer executor, not this struct.
///
/// # Example
///
/// ```rust,ignore
/// let req = TransferRequest {
///     token_id:  42,
///     from:      current_owner.clone(),
///     to:        new_owner.clone(),
///     timestamp: env.ledger().timestamp(),
///     memo:      Some(String::from_str(&env, "birthday gift")),
/// };
/// ```
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferRequest {
    /// On-chain token identifier for the NFT being transferred.
    pub token_id: TokenId,
    /// Sender — current owner of the token.
    pub from: Address,
    /// Recipient — address that will receive ownership of the token.
    pub to: Address,
    /// Ledger timestamp (seconds since Unix epoch) at request creation time.
    ///
    /// Set by the caller to `env.ledger().timestamp()` so that indexers can
    /// reconstruct a chronological transfer history without additional reads.
    pub timestamp: u64,
    /// Optional human-readable note attached to this transfer.
    ///
    /// Useful for gift messages, marketplace order references, or internal
    /// tracking IDs. Not persisted in on-chain storage by the executor.
    pub memo: Option<String>,
}

// ─── Batch transfer ───────────────────────────────────────────────────────────

/// Wraps multiple [`TransferRequest`]s to be processed in a single transaction.
///
/// Using a batch reduces the number of contract invocations needed when a
/// wallet or marketplace needs to move several NFTs atomically. All requests
/// share the same transaction context, so a failure in any individual transfer
/// (e.g. frozen token, unauthorized caller) should cause the entire batch to
/// be rolled back by the executor.
///
/// # Limits
///
/// The number of requests in a batch is bounded by
/// [`MAX_BATCH_TRANSFER_SIZE`] (default: 50) to keep gas costs and ledger
/// entry sizes predictable. Batches must contain at least
/// [`MIN_BATCH_TRANSFER_SIZE`] (1) request.
///
/// # Serialization
///
/// `BatchTransferRequest` is `#[contracttype]`, so it serializes to XDR and
/// can be passed directly as a Soroban contract argument.
///
/// # Example
///
/// ```rust,ignore
/// let batch = BatchTransferRequest { requests };
///
/// // Validate before executing:
/// batch.validate_batch_size(config.max_batch_transfer_size)?;
/// ```
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchTransferRequest {
    /// Ordered list of individual transfer requests.
    ///
    /// Transfers are executed in index order. The executor may short-circuit
    /// on the first failure or collect per-item results depending on the
    /// chosen execution strategy.
    pub requests: Vec<TransferRequest>,
}

impl BatchTransferRequest {
    /// Validate the batch size against a caller-supplied maximum limit.
    ///
    /// Use this overload when the limit comes from a [`Config`] value that
    /// has already been read from storage by the caller.
    ///
    /// # Arguments
    /// * `max` — Maximum number of requests permitted (typically
    ///   `config.max_batch_transfer_size` or [`MAX_BATCH_TRANSFER_SIZE`]).
    ///
    /// # Errors
    /// - [`Error::InvalidConfig`] — batch contains fewer than
    ///   [`MIN_BATCH_TRANSFER_SIZE`] requests (i.e. is empty).
    /// - [`Error::BatchLimitExceeded`] — batch exceeds `max`.
    ///
    /// # Example
    /// ```rust,ignore
    /// batch.validate_batch_size(50)?;
    /// ```
    pub fn validate_batch_size(&self, max: u32) -> Result<(), Error> {
        let len = self.requests.len();
        if len < MIN_BATCH_TRANSFER_SIZE {
            return Err(Error::InvalidConfig);
        }
        if len > max {
            return Err(Error::BatchTransferLimitExceeded);
        }
        Ok(())
    }

    /// Validate the batch size by reading the limit from the contract
    /// environment, falling back to [`MAX_BATCH_TRANSFER_SIZE`] if no
    /// config has been stored yet.
    ///
    /// Prefer this overload inside contract entry points where `env` is
    /// already available.
    ///
    /// # Errors
    /// - [`Error::InvalidConfig`] — batch is empty.
    /// - [`Error::BatchLimitExceeded`] — batch exceeds the configured limit.
    pub fn validate_against_env(&self, env: &Env) -> Result<(), Error> {
        let max = crate::config::get_config(env)
            .map(|c| c.max_batch_transfer_size)
            .unwrap_or(MAX_BATCH_TRANSFER_SIZE);
        self.validate_batch_size(max)
    }

    /// Return the number of transfer requests in this batch.
    pub fn len(&self) -> u32 {
        self.requests.len()
    }

    /// Return `true` if the batch contains no requests.
    pub fn is_empty(&self) -> bool {
        self.requests.len() == 0
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

    const SAMPLE_TIMESTAMP: u64 = 1_700_000_000;

    fn make_request(env: &Env, token_id: TokenId) -> TransferRequest {
        TransferRequest {
            token_id,
            from: Address::generate(env),
            to: Address::generate(env),
            timestamp: SAMPLE_TIMESTAMP,
            memo: None,
        }
    }

    fn make_batch(env: &Env, count: u32) -> BatchTransferRequest {
        let mut requests = Vec::new(env);
        for i in 0..count {
            requests.push_back(make_request(env, i));
        }
        BatchTransferRequest { requests }
    }

    // ── TransferRequest — field coverage ─────────────────────────────────────

    #[test]
    fn transfer_request_stores_all_fields() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let memo = String::from_str(&env, "gift");
        let ts = 1_700_000_000u64;

        let req = TransferRequest {
            token_id: 7,
            from: from.clone(),
            to: to.clone(),
            timestamp: ts,
            memo: Some(memo.clone()),
        };

        assert_eq!(req.token_id, 7);
        assert_eq!(req.from, from);
        assert_eq!(req.to, to);
        assert_eq!(req.timestamp, ts);
        assert_eq!(req.memo, Some(memo));
    }

    #[test]
    fn transfer_request_includes_token_id() {
        let env = Env::default();
        let req = make_request(&env, 42);
        assert_eq!(req.token_id, 42);
    }

    #[test]
    fn transfer_request_includes_sender_address() {
        let env = Env::default();
        let from = Address::generate(&env);
        let req = TransferRequest {
            token_id: 1,
            from: from.clone(),
            to: Address::generate(&env),
            timestamp: SAMPLE_TIMESTAMP,
            memo: None,
        };
        assert_eq!(req.from, from);
    }

    #[test]
    fn transfer_request_includes_recipient_address() {
        let env = Env::default();
        let to = Address::generate(&env);
        let req = TransferRequest {
            token_id: 1,
            from: Address::generate(&env),
            to: to.clone(),
            timestamp: SAMPLE_TIMESTAMP,
            memo: None,
        };
        assert_eq!(req.to, to);
    }

    #[test]
    fn transfer_request_includes_timestamp() {
        let env = Env::default();
        let ts = 1_750_000_000u64;
        let req = TransferRequest {
            token_id: 1,
            from: Address::generate(&env),
            to: Address::generate(&env),
            timestamp: ts,
            memo: None,
        };
        assert_eq!(req.timestamp, ts);
    }

    #[test]
    fn transfer_request_timestamp_zero_is_valid() {
        // Edge case: genesis / uninitialized ledger
        let env = Env::default();
        let req = TransferRequest {
            token_id: 0,
            from: Address::generate(&env),
            to: Address::generate(&env),
            timestamp: 0,
            memo: None,
        };
        assert_eq!(req.timestamp, 0);
    }

    #[test]
    fn transfer_request_memo_is_optional() {
        let env = Env::default();
        let req = make_request(&env, 1);
        assert!(req.memo.is_none());
    }

    #[test]
    fn transfer_request_clone_is_independent() {
        let env = Env::default();
        let req = make_request(&env, 5);
        let cloned = req.clone();
        assert_eq!(req, cloned);
    }

    #[test]
    fn transfer_request_equality_requires_matching_timestamp() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);

        let req_a = TransferRequest {
            token_id: 1,
            from: from.clone(),
            to: to.clone(),
            timestamp: 100,
            memo: None,
        };
        let req_b = TransferRequest {
            token_id: 1,
            from: from.clone(),
            to: to.clone(),
            timestamp: 200,
            memo: None,
        };

        assert_ne!(req_a, req_b);
    }

    // ── BatchTransferRequest — construction ───────────────────────────────────

    #[test]
    fn batch_with_single_request_is_valid() {
        let env = Env::default();
        let batch = make_batch(&env, 1);
        assert_eq!(batch.len(), 1);
        assert!(!batch.is_empty());
    }

    #[test]
    fn batch_with_max_requests_is_valid() {
        let env = Env::default();
        let batch = make_batch(&env, MAX_BATCH_TRANSFER_SIZE);
        assert!(batch.validate_batch_size(MAX_BATCH_TRANSFER_SIZE).is_ok());
    }

    #[test]
    fn batch_preserves_request_order() {
        let env = Env::default();
        let batch = make_batch(&env, 3);
        assert_eq!(batch.requests.get(0).unwrap().token_id, 0);
        assert_eq!(batch.requests.get(1).unwrap().token_id, 1);
        assert_eq!(batch.requests.get(2).unwrap().token_id, 2);
    }

    #[test]
    fn batch_clone_is_independent() {
        let env = Env::default();
        let batch = make_batch(&env, 2);
        let cloned = batch.clone();
        assert_eq!(batch, cloned);
    }

    // ── BatchTransferRequest — validate_batch_size ────────────────────────────

    #[test]
    fn empty_batch_returns_invalid_config() {
        let env = Env::default();
        let batch = make_batch(&env, 0);
        assert_eq!(
            batch.validate_batch_size(MAX_BATCH_TRANSFER_SIZE),
            Err(Error::InvalidConfig)
        );
    }

    #[test]
    fn batch_over_limit_returns_batch_limit_exceeded() {
        let env = Env::default();
        let batch = make_batch(&env, MAX_BATCH_TRANSFER_SIZE + 1);
        assert_eq!(
            batch.validate_batch_size(MAX_BATCH_TRANSFER_SIZE),
            Err(Error::BatchLimitExceeded)
        );
    }

    #[test]
    fn batch_exactly_at_limit_is_ok() {
        let env = Env::default();
        let batch = make_batch(&env, MAX_BATCH_TRANSFER_SIZE);
        assert!(batch.validate_batch_size(MAX_BATCH_TRANSFER_SIZE).is_ok());
    }

    #[test]
    fn batch_below_custom_limit_is_ok() {
        let env = Env::default();
        let batch = make_batch(&env, 5);
        assert!(batch.validate_batch_size(10).is_ok());
    }

    #[test]
    fn batch_over_custom_limit_fails() {
        let env = Env::default();
        let batch = make_batch(&env, 11);
        assert_eq!(
            batch.validate_batch_size(10),
            Err(Error::BatchLimitExceeded)
        );
    }

    // ── BatchTransferRequest — helpers ────────────────────────────────────────

    #[test]
    fn is_empty_returns_true_for_zero_requests() {
        let env = Env::default();
        let batch = make_batch(&env, 0);
        assert!(batch.is_empty());
    }

    #[test]
    fn is_empty_returns_false_for_non_empty_batch() {
        let env = Env::default();
        let batch = make_batch(&env, 3);
        assert!(!batch.is_empty());
    }

    #[test]
    fn len_matches_request_count() {
        let env = Env::default();
        for n in [1u32, 5, 10, 25, 50] {
            let batch = make_batch(&env, n);
            assert_eq!(batch.len(), n);
        }
    }
}
