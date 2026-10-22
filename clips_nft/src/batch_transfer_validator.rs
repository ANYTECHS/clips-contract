//! Batch transfer validator (issue #1198).
//!
//! Validates batch NFT transfer requests before execution. A batch transfer
//! packages multiple individual transfers into one atomic call; this validator
//! ensures every structural constraint is satisfied before any storage mutation
//! occurs.
//!
//! # Acceptance criteria (issue #1198)
//!
//! | Criterion | Implementation |
//! |-----------|----------------|
//! | Validate token collection | [`validate_token_ids`] |
//! | Validate recipient collection | [`validate_recipients`] |
//! | Ensure matching collection sizes | [`validate_collection_sizes`] |
//! | Enforce batch size limits | [`validate_batch_size`] |
//! | Add batch validation tests | `tests` module below |
//!
//! # Errors
//!
//! - [`Error::EmptyBatch`] — the batch contains no transfer requests.
//! - [`Error::BatchTooLarge`] — the batch exceeds [`MAX_BATCH_TRANSFER_SIZE`].
//! - [`Error::InvalidTokenId`] — a token ID of `0` (reserved sentinel) appears in the batch.
//! - [`Error::DuplicateToken`] — the same token ID appears more than once in the batch.
//! - [`Error::InvalidBatchRecipient`] — a recipient address is the current contract address.
//! - [`Error::InvalidBatchRequest`] — the token and recipient collections differ in length.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::batch_transfer_validator::{
//!     validate_batch_transfer, BatchTransferInput, BatchTransferValidator,
//! };
//! use crate::validators::{run_validator, ValidationContext};
//!
//! // Functional API:
//! validate_batch_transfer(env, &token_ids, &recipients)?;
//!
//! // Pipeline API:
//! let ctx = ValidationContext::new(env);
//! let input = BatchTransferInput { token_ids: token_ids.clone(), recipients: recipients.clone() };
//! run_validator(&BatchTransferValidator, &ctx, &input)?;
//! ```

use soroban_sdk::{Address, Env, Vec};

use crate::storage_constants::{MAX_BATCH_TRANSFER_SIZE, MIN_BATCH_TRANSFER_SIZE};
use crate::types::{Error, TokenId};
use crate::validators::{ValidationContext, ValidationResult, Validator};

// ─── Input type ───────────────────────────────────────────────────────────────

/// Input bundle for the standardized [`BatchTransferValidator`].
///
/// Groups the token ID collection and recipient collection together so the
/// [`Validator<BatchTransferInput>`] interface receives a single typed value.
pub struct BatchTransferInput {
    /// Ordered list of token IDs to transfer.
    pub token_ids: Vec<TokenId>,
    /// Ordered list of recipient addresses, one per token.
    pub recipients: Vec<Address>,
}

// ─── Individual validators ────────────────────────────────────────────────────

/// Validate the batch size against the configured limits.
///
/// The batch must contain at least [`MIN_BATCH_TRANSFER_SIZE`] request (1) and
/// must not exceed [`MAX_BATCH_TRANSFER_SIZE`] (50).
///
/// # Errors
/// - [`Error::EmptyBatch`] — fewer than [`MIN_BATCH_TRANSFER_SIZE`] entries.
/// - [`Error::BatchTooLarge`] — more than [`MAX_BATCH_TRANSFER_SIZE`] entries.
pub fn validate_batch_size(count: u32) -> Result<(), Error> {
    if count < MIN_BATCH_TRANSFER_SIZE {
        return Err(Error::EmptyBatch);
    }
    if count > MAX_BATCH_TRANSFER_SIZE {
        return Err(Error::BatchTooLarge);
    }
    Ok(())
}

/// Validate the token ID collection in the batch.
///
/// Each token ID must be non-zero (token ID `0` is the reserved sentinel
/// meaning "unset" across the contract). The collection must also contain no
/// duplicates — the same token cannot appear in more than one slot of the batch.
///
/// # Errors
/// - [`Error::InvalidTokenId`] — a token ID of `0` appears in the collection.
/// - [`Error::DuplicateToken`] — the same token ID appears more than once.
pub fn validate_token_ids(env: &Env, token_ids: &Vec<TokenId>) -> Result<(), Error> {
    let mut seen: Vec<TokenId> = Vec::new(env);
    for token_id in token_ids.iter() {
        if token_id == 0 {
            return Err(Error::InvalidTokenId);
        }
        if seen.contains(&token_id) {
            return Err(Error::DuplicateToken);
        }
        seen.push_back(token_id);
    }
    Ok(())
}

/// Validate the recipient collection in the batch.
///
/// Each recipient must be a real external address. The current contract
/// address is rejected to prevent tokens being locked inside the contract
/// itself.
///
/// # Errors
/// - [`Error::InvalidBatchRecipient`] — a recipient is the current contract address.
pub fn validate_recipients(env: &Env, recipients: &Vec<Address>) -> Result<(), Error> {
    let contract_addr = env.current_contract_address();
    for recipient in recipients.iter() {
        if recipient == contract_addr {
            return Err(Error::InvalidBatchRecipient);
        }
    }
    Ok(())
}

/// Validate that the token ID and recipient collections are the same length.
///
/// Every token transfer in a batch must have exactly one recipient. A mismatch
/// indicates a malformed request.
///
/// # Errors
/// - [`Error::InvalidBatchRequest`] — `token_count != recipient_count`.
pub fn validate_collection_sizes(token_count: u32, recipient_count: u32) -> Result<(), Error> {
    if token_count != recipient_count {
        return Err(Error::InvalidBatchRequest);
    }
    Ok(())
}

/// Run all batch transfer validation checks in order.
///
/// This is the primary entry point. The checks run as follows:
/// 1. Validate batch size (`validate_batch_size`).
/// 2. Validate collection size parity (`validate_collection_sizes`).
/// 3. Validate the token ID collection (`validate_token_ids`).
/// 4. Validate the recipient collection (`validate_recipients`).
///
/// # Errors
/// - [`Error::EmptyBatch`] — batch contains no entries.
/// - [`Error::BatchTooLarge`] — batch exceeds [`MAX_BATCH_TRANSFER_SIZE`].
/// - [`Error::InvalidBatchRequest`] — token and recipient collections differ in length.
/// - [`Error::InvalidTokenId`] — a token ID of `0` appears in the batch.
/// - [`Error::DuplicateToken`] — a token ID is repeated.
/// - [`Error::InvalidBatchRecipient`] — a recipient is the contract address.
pub fn validate_batch_transfer(
    env: &Env,
    token_ids: &Vec<TokenId>,
    recipients: &Vec<Address>,
) -> Result<(), Error> {
    let token_count = token_ids.len();
    let recipient_count = recipients.len();

    validate_batch_size(token_count)?;
    validate_collection_sizes(token_count, recipient_count)?;
    validate_token_ids(env, token_ids)?;
    validate_recipients(env, recipients)?;
    Ok(())
}

// ─── Standardized Validator<BatchTransferInput> implementation ────────────────

/// Standardized batch transfer validator implementing
/// [`Validator<BatchTransferInput>`].
///
/// Wraps [`validate_batch_transfer`] so it can be composed inside validation
/// pipelines using [`crate::validators::run_validator`] and
/// [`crate::validators::validate_all`].
///
/// # Usage
/// ```rust,ignore
/// use crate::validators::{run_validator, ValidationContext};
/// use crate::batch_transfer_validator::{BatchTransferInput, BatchTransferValidator};
///
/// let ctx = ValidationContext::new(&env);
/// let input = BatchTransferInput { token_ids: ids.clone(), recipients: recips.clone() };
/// run_validator(&BatchTransferValidator, &ctx, &input)?;
/// ```
pub struct BatchTransferValidator;

impl Validator<BatchTransferInput> for BatchTransferValidator {
    /// Validate the batch transfer `input` against all structural constraints.
    ///
    /// Returns [`ValidationResult::Valid`] when all checks pass, or
    /// [`ValidationResult::Invalid`] with the first failing error.
    fn validate(
        &self,
        ctx: &ValidationContext<'_>,
        input: &BatchTransferInput,
    ) -> ValidationResult {
        ValidationResult::from_result(validate_batch_transfer(
            ctx.env,
            &input.token_ids,
            &input.recipients,
        ))
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage_constants::MAX_BATCH_TRANSFER_SIZE;
    use crate::validators::{run_validator, ValidationContext};
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn make_token_ids(env: &Env, ids: &[TokenId]) -> Vec<TokenId> {
        let mut v = Vec::new(env);
        for &id in ids {
            v.push_back(id);
        }
        v
    }

    fn make_recipients(env: &Env, count: u32) -> Vec<Address> {
        let mut v = Vec::new(env);
        for _ in 0..count {
            v.push_back(Address::generate(env));
        }
        v
    }

    // ── validate_batch_size ───────────────────────────────────────────────────

    #[test]
    fn accepts_minimum_batch_size() {
        assert!(validate_batch_size(1).is_ok());
    }

    #[test]
    fn accepts_typical_batch_size() {
        assert!(validate_batch_size(10).is_ok());
    }

    #[test]
    fn accepts_maximum_batch_size() {
        assert!(validate_batch_size(MAX_BATCH_TRANSFER_SIZE).is_ok());
    }

    #[test]
    fn rejects_empty_batch() {
        assert_eq!(validate_batch_size(0), Err(Error::EmptyBatch));
    }

    #[test]
    fn rejects_batch_exceeding_max_size() {
        assert_eq!(
            validate_batch_size(MAX_BATCH_TRANSFER_SIZE + 1),
            Err(Error::BatchTooLarge)
        );
    }

    // ── validate_token_ids ────────────────────────────────────────────────────

    #[test]
    fn accepts_valid_distinct_token_ids() {
        with_contract(|env| {
            let ids = make_token_ids(env, &[1, 2, 3]);
            assert!(validate_token_ids(env, &ids).is_ok());
        });
    }

    #[test]
    fn rejects_token_id_zero() {
        with_contract(|env| {
            let ids = make_token_ids(env, &[0, 1, 2]);
            assert_eq!(validate_token_ids(env, &ids), Err(Error::InvalidTokenId));
        });
    }

    #[test]
    fn rejects_token_id_zero_in_any_position() {
        with_contract(|env| {
            let ids = make_token_ids(env, &[1, 0, 2]);
            assert_eq!(validate_token_ids(env, &ids), Err(Error::InvalidTokenId));
        });
    }

    #[test]
    fn rejects_duplicate_token_ids() {
        with_contract(|env| {
            let ids = make_token_ids(env, &[1, 2, 1]);
            assert_eq!(validate_token_ids(env, &ids), Err(Error::DuplicateToken));
        });
    }

    #[test]
    fn rejects_all_same_token_ids() {
        with_contract(|env| {
            let ids = make_token_ids(env, &[5, 5, 5]);
            assert_eq!(validate_token_ids(env, &ids), Err(Error::DuplicateToken));
        });
    }

    // ── validate_recipients ───────────────────────────────────────────────────

    #[test]
    fn accepts_valid_recipients() {
        with_contract(|env| {
            let recipients = make_recipients(env, 3);
            assert!(validate_recipients(env, &recipients).is_ok());
        });
    }

    #[test]
    fn rejects_contract_address_as_recipient() {
        with_contract(|env| {
            let mut recipients = make_recipients(env, 2);
            recipients.push_back(env.current_contract_address());
            assert_eq!(
                validate_recipients(env, &recipients),
                Err(Error::InvalidBatchRecipient)
            );
        });
    }

    #[test]
    fn rejects_contract_as_only_recipient() {
        with_contract(|env| {
            let mut recipients = Vec::new(env);
            recipients.push_back(env.current_contract_address());
            assert_eq!(
                validate_recipients(env, &recipients),
                Err(Error::InvalidBatchRecipient)
            );
        });
    }

    // ── validate_collection_sizes ─────────────────────────────────────────────

    #[test]
    fn matching_sizes_are_accepted() {
        assert!(validate_collection_sizes(3, 3).is_ok());
    }

    #[test]
    fn zero_zero_is_accepted() {
        assert!(validate_collection_sizes(0, 0).is_ok());
    }

    #[test]
    fn more_tokens_than_recipients_is_rejected() {
        assert_eq!(
            validate_collection_sizes(3, 2),
            Err(Error::InvalidBatchRequest)
        );
    }

    #[test]
    fn more_recipients_than_tokens_is_rejected() {
        assert_eq!(
            validate_collection_sizes(2, 3),
            Err(Error::InvalidBatchRequest)
        );
    }

    // ── validate_batch_transfer (full pipeline) ───────────────────────────────

    #[test]
    fn valid_batch_passes_all_checks() {
        with_contract(|env| {
            let token_ids = make_token_ids(env, &[1, 2, 3]);
            let recipients = make_recipients(env, 3);
            assert!(validate_batch_transfer(env, &token_ids, &recipients).is_ok());
        });
    }

    #[test]
    fn empty_batch_fails_on_batch_size() {
        with_contract(|env| {
            let token_ids = make_token_ids(env, &[]);
            let recipients = make_recipients(env, 0);
            assert_eq!(
                validate_batch_transfer(env, &token_ids, &recipients),
                Err(Error::EmptyBatch)
            );
        });
    }

    #[test]
    fn size_mismatch_fails_on_collection_sizes() {
        with_contract(|env| {
            let token_ids = make_token_ids(env, &[1, 2]);
            let recipients = make_recipients(env, 3);
            assert_eq!(
                validate_batch_transfer(env, &token_ids, &recipients),
                Err(Error::InvalidBatchRequest)
            );
        });
    }

    #[test]
    fn duplicate_token_fails_on_token_ids() {
        with_contract(|env| {
            let token_ids = make_token_ids(env, &[1, 1]);
            let recipients = make_recipients(env, 2);
            assert_eq!(
                validate_batch_transfer(env, &token_ids, &recipients),
                Err(Error::DuplicateToken)
            );
        });
    }

    #[test]
    fn contract_recipient_fails_on_recipients() {
        with_contract(|env| {
            let token_ids = make_token_ids(env, &[1]);
            let mut recipients = Vec::new(env);
            recipients.push_back(env.current_contract_address());
            assert_eq!(
                validate_batch_transfer(env, &token_ids, &recipients),
                Err(Error::InvalidBatchRecipient)
            );
        });
    }

    // ── BatchTransferValidator (Validator<BatchTransferInput> interface) ───────

    #[test]
    fn validator_interface_accepts_valid_input() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            let input = BatchTransferInput {
                token_ids: make_token_ids(env, &[1, 2, 3]),
                recipients: make_recipients(env, 3),
            };
            assert!(run_validator(&BatchTransferValidator, &ctx, &input).is_ok());
        });
    }

    #[test]
    fn validator_interface_rejects_empty_batch() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            let input = BatchTransferInput {
                token_ids: make_token_ids(env, &[]),
                recipients: make_recipients(env, 0),
            };
            assert_eq!(
                run_validator(&BatchTransferValidator, &ctx, &input),
                Err(Error::EmptyBatch)
            );
        });
    }

    #[test]
    fn validator_interface_rejects_oversized_batch() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            let ids: Vec<TokenId> = {
                let mut v = Vec::new(env);
                for i in 1..=(MAX_BATCH_TRANSFER_SIZE + 1) {
                    v.push_back(i);
                }
                v
            };
            let recipients = make_recipients(env, MAX_BATCH_TRANSFER_SIZE + 1);
            let input = BatchTransferInput {
                token_ids: ids,
                recipients,
            };
            assert_eq!(
                run_validator(&BatchTransferValidator, &ctx, &input),
                Err(Error::BatchTooLarge)
            );
        });
    }

    #[test]
    fn validator_interface_rejects_duplicate_tokens() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            let input = BatchTransferInput {
                token_ids: make_token_ids(env, &[1, 1]),
                recipients: make_recipients(env, 2),
            };
            assert_eq!(
                run_validator(&BatchTransferValidator, &ctx, &input),
                Err(Error::DuplicateToken)
            );
        });
    }
}
