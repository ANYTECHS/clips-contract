# ClipCash Error Catalog

Contract errors use a single `clips_nft::types::Error` enum. The enum discriminant is the on-chain error code; the same standardized name and category are published through `clips_nft::error_infrastructure`.

## Naming convention

- Use `PascalCase` and describe the failed invariant, not the implementation.
- State errors describe lifecycle state: `AlreadyBurned`, `AlreadyFrozen`, `AlreadyUnfrozen`, and `InvalidLifecycleTransition`.
- Authorization errors describe permission: `UnauthorizedTransfer`, `OperatorNotApproved`, `ApprovalNotFound`, and `ApprovalAlreadyExists`.
- Transfer-state errors describe an unusable token or ownership record: `InvalidTransferState`, `MissingOwner`, and `InvalidOwnershipState`.
- Batch errors describe the request as a whole: `EmptyBatch`, `BatchTooLarge`, `InvalidBatchRequest`, and `DuplicateToken`.

## Code allocation

| Range | Category |
| --- | --- |
| 200–204 | Initialization |
| 210–214 | Configuration |
| 220–225 | Validation |
| 230–231 | Core storage and ownership |
| 240–242 | Transfer recipient and frozen-token validation |
| 250 | Minting |
| 260–263 | NFT lifecycle |
| 270–273 | Approval and transfer authorization |
| 280–284 | Transfer state |
| 290–293 | Batch transfers |

Every registry entry has one unique name, one unique code, a module, and a description. Call `error_infrastructure::error_code`, `error_infrastructure::name_for`, and `error_catalog::categorize_by_code` instead of duplicating code mappings in integrations.
