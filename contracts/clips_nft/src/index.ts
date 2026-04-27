import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}




/**
 * Custom errors for the NFT contract
 */
export const Errors = {
  /**
   * Operation not authorized
   */
  1: {message:"Unauthorized"},
  /**
   * Invalid token ID
   */
  2: {message:"InvalidTokenId"},
  /**
   * Token already minted
   */
  3: {message:"TokenAlreadyMinted"},
  /**
   * Royalty too high (max 10000 basis points = 100%)
   */
  4: {message:"RoyaltyTooHigh"},
  /**
   * Invalid recipient
   */
  5: {message:"InvalidRecipient"},
  /**
   * Sale price must be greater than zero
   */
  6: {message:"InvalidSalePrice"},
  /**
   * Contract is paused — minting and transfers are blocked
   */
  7: {message:"ContractPaused"},
  /**
   * Backend signature over the mint payload is invalid
   */
  8: {message:"InvalidSignature"},
  /**
   * No backend signer public key has been registered yet
   */
  9: {message:"SignerNotSet"},
  /**
   * Royalty split is invalid
   */
  10: {message:"InvalidRoyaltySplit"},
  /**
   * Token is soulbound (non-transferable)
   */
  11: {message:"SoulboundTransferBlocked"},
  /**
   * Royalty calculation would overflow
   */
  12: {message:"RoyaltyOverflow"},
  /**
   * Clip is blacklisted
   */
  13: {message:"ClipBlacklisted"},
  /**
   * Caller is not authorized to approve
   */
  14: {message:"NotAuthorizedToApprove"},
  /**
   * Withdrawal is still locked (24h safety delay)
   */
  15: {message:"WithdrawalStillLocked"},
  /**
   * No active withdrawal request found
   */
  16: {message:"NoWithdrawalRequest"}
}

/**
 * Storage keys
 * 
 * Key sizing notes:
 * - Enum variants with no payload (Admin, NextTokenId, Paused) are 1-word keys.
 * - Variants with a u32 payload (Token, ClipIdMinted) are
 * 2-word keys — the smallest possible for per-token entries.
 */
export type DataKey = {tag: "Admin", values: void} | {tag: "NextTokenId", values: void} | {tag: "Paused", values: void} | {tag: "Token", values: readonly [TokenId]} | {tag: "ClipIdMinted", values: readonly [u32]} | {tag: "Signer", values: void} | {tag: "PlatformRecipient", values: void} | {tag: "TotalGasMint", values: void} | {tag: "CountMint", values: void} | {tag: "TotalGasTransfer", values: void} | {tag: "CountTransfer", values: void};


export interface Royalty {
  /**
 * Optional SEP-0041 asset contract address.
 * `None` → royalties expected in XLM (native).
 */
asset_address: Option<string>;
  /**
 * Multi-recipient split. Platform recipient is automatically added with 1%
 * if not present.
 */
recipients: Array<RoyaltyRecipient>;
}


/**
 * Event emitted when an NFT is burned.
 */
export interface BurnEvent {
  clip_id: u32;
  owner: string;
  token_id: TokenId;
}


/**
 * Event emitted when a new NFT is minted
 */
export interface MintEvent {
  clip_id: u32;
  gas_used: u64;
  metadata_uri: string;
  to: string;
  token_id: TokenId;
}


/**
 * Packs owner address, originating clip_id, metadata, and royalty into a single persistent entry.
 * 
 * Combining these fields eliminates the separate `Metadata` and `Royalty`
 * entries that were previously written on every mint.
 */
export interface TokenData {
  /**
 * The off-chain clip identifier this token was minted for.
 */
clip_id: u32;
  /**
 * Whether this token is soulbound (non-transferable)
 */
is_soulbound: boolean;
  /**
 * Metadata URI for the token
 */
metadata_uri: string;
  owner: string;
  /**
 * Royalty configuration
 */
royalty: Royalty;
}


/**
 * Royalty payment info returned by `royalty_info()`.
 */
export interface RoyaltyInfo {
  /**
 * `None` → pay in XLM; `Some(addr)` → pay in that SEP-0041 token.
 */
asset_address: Option<string>;
  receiver: string;
  /**
 * Royalty amount in the same denomination as `sale_price`
 */
royalty_amount: i128;
}


/**
 * Event emitted when NFT ownership changes.
 */
export interface TransferEvent {
  from: string;
  gas_used: u64;
  to: string;
  token_id: TokenId;
}


/**
 * Event emitted when royalty is paid.
 */
export interface RoyaltyPaidEvent {
  amount: i128;
  from: string;
  to: string;
  token_id: TokenId;
}


/**
 * Royalty information stored per token.
 * `asset_address` is `None` for native XLM, or `Some(contract_address)`
 * for any SEP-0041 custom Stellar asset.
 */
export interface RoyaltyRecipient {
  basis_points: u32;
  recipient: string;
}


/**
 * Event emitted when royalty recipient is updated.
 */
export interface RoyaltyRecipientUpdatedEvent {
  new_recipient: string;
  old_recipient: string;
  token_id: TokenId;
}

/**
 * Event emitted when a clip ID is blacklisted by admin.
 */
export interface BlacklistEvent {
  clip_id: u32;
}

export interface Client {
  /**
   * Construct and simulate a burn transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Burn (destroy) an NFT. Only the current owner may burn.
   * 
   * Storage removes (persistent): TokenData, ClipIdMinted = **2** (Optimized from 4)
   */
  burn: ({owner, token_id}: {owner: string, token_id: TokenId}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a init transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initialize the contract with an admin address.
   */
  init: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a mint transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Mint a new NFT for a video clip.
   * 
   * Requires a valid Ed25519 `signature` from the registered backend signer
   * over the canonical mint payload, proving the clip exists and belongs to
   * `to`. The payload is:
   * 
   * ```text
   * payload = SHA-256(
   * clip_id_le_4_bytes
   * || SHA-256(owner_address_xdr)   // 32 bytes
   * || SHA-256(metadata_uri_bytes)  // 32 bytes
   * )
   * ```
   * 
   * Storage writes (persistent): TokenData, Metadata, Royalty, ClipIdMinted = **4**
   * Instance writes: NextTokenId = **1**
   * 
   * # Arguments
   * * `to`           - Address that will own the NFT (must match the signed payload)
   * * `clip_id`      - Unique off-chain clip identifier (must match the signed payload)
   * * `metadata_uri` - IPFS or Arweave URI (must match the signed payload)
   * * `royalty`      - Royalty configuration
   * * `is_soulbound` - Whether the token is soulbound (non-transferable)
   * * `signature`    - 64-byte Ed25519 signature from the backend signer
   */
  mint: ({to, clip_id, metadata_uri, royalty, is_soulbound, signature}: {to: string, clip_id: u32, metadata_uri: string, royalty: Royalty, is_soulbound: boolean, signature: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<TokenId>>>

  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pause the contract. Blocks `mint` and `transfer` until unpaused.
   * Only callable by the admin.
   */
  pause: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a exists transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns true if the token exists.
   */
  exists: ({token_id}: {token_id: TokenId}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Unpause the contract, re-enabling `mint` and `transfer`.
   * Only callable by the admin.
   */
  unpause: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a version transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the contract version.
   */
  version: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a owner_of transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the owner of a given token ID.
   */
  owner_of: ({token_id}: {token_id: TokenId}, options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a transfer transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Transfer NFT ownership from `from` to `to`.
   * 
   * Blocked if the token is soulbound (non-transferable).
   * Storage writes (persistent): TokenData = **1**
   * 
   * # Arguments
   * * `from`     - Current owner (must authorize)
   * * `to`       - New owner
   * * `token_id` - Token to transfer
   */
  transfer: ({from, to, token_id}: {from: string, to: string, token_id: TokenId}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a is_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns `true` if the contract is currently paused.
   */
  is_paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a token_uri transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the metadata URI for a given token ID.
   */
  token_uri: ({token_id}: {token_id: TokenId}, options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a get_signer transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return the currently registered backend signer public key, if any.
   */
  get_signer: (options?: MethodOptions) => Promise<AssembledTransaction<Option<Buffer>>>

  /**
   * Construct and simulate a set_signer transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Register (or rotate) the backend Ed25519 public key used to verify
   * clip ownership before minting. Only callable by the admin.
   * 
   * # Arguments
   * * `admin`  - Must be the contract admin
   * * `pubkey` - 32-byte Ed25519 public key of the trusted backend signer
   */
  set_signer: ({admin, pubkey}: {admin: string, pubkey: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_royalty transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the stored `Royalty` struct for a token.
   */
  get_royalty: ({token_id}: {token_id: TokenId}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Royalty>>>

  /**
   * Construct and simulate a pay_royalty transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pay royalties for a token sale using the asset configured in the royalty.
   * 
   * Only handles SEP-0041 custom assets. For XLM (`asset_address` is `None`)
   * the marketplace must handle the transfer directly.
   */
  pay_royalty: ({payer, token_id, sale_price}: {payer: string, token_id: TokenId, sale_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_royalty transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Update the royalty configuration for a token. Admin only.
   * Emits RoyaltyRecipientUpdated event when the primary recipient changes.
   */
  set_royalty: ({admin, token_id, new_royalty}: {admin: string, token_id: TokenId, new_royalty: Royalty}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_metadata transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Alias for `token_uri`, kept for compatibility.
   */
  get_metadata: ({token_id}: {token_id: TokenId}, options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a is_soulbound transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns true if the token is soulbound (non-transferable).
   */
  is_soulbound: ({token_id}: {token_id: TokenId}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a royalty_info transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the royalty receiver, amount, and payment asset for a given sale price.
   * 
   * Uses safe math to prevent overflow. Royalty amount is calculated as:
   * `royalty_amount = sale_price * basis_points / 10000`
   * 
   * Safe limits: sale_price should not exceed i128::MAX / 10000 to avoid overflow.
   */
  royalty_info: ({token_id, sale_price}: {token_id: TokenId, sale_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<RoyaltyInfo>>>

  /**
   * Construct and simulate a total_supply transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the total number of minted (and not yet burned) tokens.
   * 
   * Derived from `NextTokenId - 1` — no separate counter needed.
   */
  total_supply: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a clip_token_id transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Look up the on-chain token ID for a given clip_id.
   */
  clip_token_id: ({clip_id}: {clip_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<TokenId>>>

  /**
   * Construct and simulate a get_avg_gas_cost transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the average synthetic gas cost for a given operation type.
   * 0 = Mint, 1 = Transfer
   */
  get_avg_gas_cost: ({op_type}: {op_type: u32}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a blacklist_clip transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Blacklist a clip ID, preventing it from being minted. Only callable by the admin.
   * Emits a Blacklist event.
   */
  blacklist_clip: ({admin, clip_id}: {admin: string, clip_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate an update_royalty_recipient transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Allow the current royalty recipient to update their address.
   * Only the current primary royalty recipient (index 0) may call this.
   * Emits RoyaltyRecipientUpdated event.
   */
  update_royalty_recipient: ({caller, token_id, new_recipient}: {caller: string, token_id: TokenId, new_recipient: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a tokens_of_owner transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return all token IDs owned by `owner`. Capped at 1000 results.
   */
  tokens_of_owner: ({owner}: {owner: string}, options?: MethodOptions) => Promise<AssembledTransaction<Array<TokenId>>>

  /**
   * Construct and simulate a calculate_royalty_amount transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Calculate the royalty amount for a given sale price using the token's stored royalty configuration.
   */
  calculate_royalty_amount: ({token_id, sale_price}: {token_id: TokenId, sale_price: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a batch_mint transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Mint multiple clips in a single transaction.
   */
  batch_mint: ({to, clip_ids, metadata_uris, royalty, is_soulbound, signatures}: {to: string, clip_ids: Array<u32>, metadata_uris: Array<string>, royalty: Royalty, is_soulbound: boolean, signatures: Array<Buffer>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Array<TokenId>>>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAACJDdXN0b20gZXJyb3JzIGZvciB0aGUgTkZUIGNvbnRyYWN0AAAAAAAAAAAABUVycm9yAAAAAAAADAAAABhPcGVyYXRpb24gbm90IGF1dGhvcml6ZWQAAAAMVW5hdXRob3JpemVkAAAAAQAAABBJbnZhbGlkIHRva2VuIElEAAAADkludmFsaWRUb2tlbklkAAAAAAACAAAAFFRva2VuIGFscmVhZHkgbWludGVkAAAAElRva2VuQWxyZWFkeU1pbnRlZAAAAAAAAwAAADBSb3lhbHR5IHRvbyBoaWdoIChtYXggMTAwMDAgYmFzaXMgcG9pbnRzID0gMTAwJSkAAAAOUm95YWx0eVRvb0hpZ2gAAAAAAAQAAAARSW52YWxpZCByZWNpcGllbnQAAAAAAAAQSW52YWxpZFJlY2lwaWVudAAAAAUAAAAkU2FsZSBwcmljZSBtdXN0IGJlIGdyZWF0ZXIgdGhhbiB6ZXJvAAAAEEludmFsaWRTYWxlUHJpY2UAAAAGAAAAOENvbnRyYWN0IGlzIHBhdXNlZCDigJQgbWludGluZyBhbmQgdHJhbnNmZXJzIGFyZSBibG9ja2VkAAAADkNvbnRyYWN0UGF1c2VkAAAAAAAHAAAAMkJhY2tlbmQgc2lnbmF0dXJlIG92ZXIgdGhlIG1pbnQgcGF5bG9hZCBpcyBpbnZhbGlkAAAAAAAQSW52YWxpZFNpZ25hdHVyZQAAAAgAAAA0Tm8gYmFja2VuZCBzaWduZXIgcHVibGljIGtleSBoYXMgYmVlbiByZWdpc3RlcmVkIHlldAAAAAxTaWduZXJOb3RTZXQAAAAJAAAAGFJveWFsdHkgc3BsaXQgaXMgaW52YWxpZAAAABNJbnZhbGlkUm95YWx0eVNwbGl0AAAAAAoAAAAlVG9rZW4gaXMgc291bGJvdW5kIChub24tdHJhbnNmZXJhYmxlKQAAAAAAABhTb3VsYm91bmRUcmFuc2ZlckJsb2NrZWQAAAALAAAAIlJveWFsdHkgY2FsY3VsYXRpb24gd291bGQgb3ZlcmZsb3cAAAAAAA9Sb3lhbHR5T3ZlcmZsb3cAAAAADA==",
        "AAAAAgAAAOJTdG9yYWdlIGtleXMKCktleSBzaXppbmcgbm90ZXM6Ci0gRW51bSB2YXJpYW50cyB3aXRoIG5vIHBheWxvYWQgKEFkbWluLCBOZXh0VG9rZW5JZCwgUGF1c2VkKSBhcmUgMS13b3JkIGtleXMuCi0gVmFyaWFudHMgd2l0aCBhIHUzMiBwYXlsb2FkIChUb2tlbiwgQ2xpcElkTWludGVkKSBhcmUKMi13b3JkIGtleXMg4oCUIHRoZSBzbWFsbGVzdCBwb3NzaWJsZSBmb3IgcGVyLXRva2VuIGVudHJpZXMuAAAAAAAAAAAAB0RhdGFLZXkAAAAACwAAAAAAAAAxQ29udHJhY3QgYWRtaW5pc3RyYXRvciBhZGRyZXNzIChpbnN0YW5jZSBzdG9yYWdlKQAAAAAAAAVBZG1pbgAAAAAAAAAAAACBTW9ub3RvbmljYWxseSBpbmNyZWFzaW5nIHRva2VuIElEIGNvdW50ZXIgKGluc3RhbmNlIHN0b3JhZ2UpLgpgdG90YWxfc3VwcGx5ID0gTmV4dFRva2VuSWQgLSAxYCDigJQgbm8gc2VwYXJhdGUgVG9rZW5Db3VudCBuZWVkZWQuAAAAAAAAC05leHRUb2tlbklkAAAAAAAAAAAdUGF1c2UgZmxhZyAoaW5zdGFuY2Ugc3RvcmFnZSkAAAAAAAAGUGF1c2VkAAAAAAABAAAATFBhY2tlZCBvd25lciArIGNsaXBfaWQgKyBtZXRhZGF0YSArIHJveWFsdHkgZm9yIGEgdG9rZW4gKHBlcnNpc3RlbnQgc3RvcmFnZSkAAAAFVG9rZW4AAAAAAAABAAAH0AAAAAdUb2tlbklkAAAAAAEAAAA2RGVkdXAgZ3VhcmQ6IGNsaXBfaWQg4oaSIHRva2VuX2lkIChwZXJzaXN0ZW50IHN0b3JhZ2UpAAAAAAAMQ2xpcElkTWludGVkAAAAAQAAAAQAAAAAAAAAQ0VkMjU1MTkgcHVibGljIGtleSBvZiB0aGUgdHJ1c3RlZCBiYWNrZW5kIHNpZ25lciAoaW5zdGFuY2Ugc3RvcmFnZSkAAAAABlNpZ25lcgAAAAAAAAAAADJQbGF0Zm9ybSByZWNpcGllbnQgdXNlZCBmb3IgZGVmYXVsdCAxJSByb3lhbHR5IGN1dAAAAAAAEVBsYXRmb3JtUmVjaXBpZW50AAAAAAAAAAAAADZUb3RhbCBzeW50aGV0aWMgZ2FzIHVzZWQgaW4gbWludGluZyAoaW5zdGFuY2Ugc3RvcmFnZSkAAAAAAAxUb3RhbEdhc01pbnQAAAAAAAAAM1RvdGFsIG51bWJlciBvZiBzdWNjZXNzZnVsIG1pbnRzIChpbnN0YW5jZSBzdG9yYWdlKQAAAAAJQ291bnRNaW50AAAAAAAAAAAAADhUb3RhbCBzeW50aGV0aWMgZ2FzIHVzZWQgaW4gdHJhbnNmZXJzIChpbnN0YW5jZSBzdG9yYWdlKQAAABBUb3RhbEdhc1RyYW5zZmVyAAAAAAAAADdUb3RhbCBudW1iZXIgb2Ygc3VjY2Vzc2Z1bCB0cmFuc2ZlcnMgKGluc3RhbmNlIHN0b3JhZ2UpAAAAAA1Db3VudFRyYW5zZmVyAAAA",
        "AAAAAQAAAAAAAAAAAAAAB1JveWFsdHkAAAAAAgAAAFhPcHRpb25hbCBTRVAtMDA0MSBhc3NldCBjb250cmFjdCBhZGRyZXNzLgpgTm9uZWAg4oaSIHJveWFsdGllcyBleHBlY3RlZCBpbiBYTE0gKG5hdGl2ZSkuAAAADWFzc2V0X2FkZHJlc3MAAAAAAAPoAAAAEwAAAFhNdWx0aS1yZWNpcGllbnQgc3BsaXQuIFBsYXRmb3JtIHJlY2lwaWVudCBpcyBhdXRvbWF0aWNhbGx5IGFkZGVkIHdpdGggMSUKaWYgbm90IHByZXNlbnQuAAAACnJlY2lwaWVudHMAAAAAA+oAAAfQAAAAEFJveWFsdHlSZWNpcGllbnQ=",
        "AAAAAQAAACRFdmVudCBlbWl0dGVkIHdoZW4gYW4gTkZUIGlzIGJ1cm5lZC4AAAAAAAAACUJ1cm5FdmVudAAAAAAAAAMAAAAAAAAAB2NsaXBfaWQAAAAABAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAh0b2tlbl9pZAAAB9AAAAAHVG9rZW5JZAA=",
        "AAAAAQAAACZFdmVudCBlbWl0dGVkIHdoZW4gYSBuZXcgTkZUIGlzIG1pbnRlZAAAAAAAAAAAAAlNaW50RXZlbnQAAAAAAAAFAAAAAAAAAAdjbGlwX2lkAAAAAAQAAAAAAAAACGdhc191c2VkAAAABgAAAAAAAAAMbWV0YWRhdGFfdXJpAAAAEAAAAAAAAAACdG8AAAAAABMAAAAAAAAACHRva2VuX2lkAAAH0AAAAAdUb2tlbklkAA==",
        "AAAAAQAAANxQYWNrcyBvd25lciBhZGRyZXNzLCBvcmlnaW5hdGluZyBjbGlwX2lkLCBtZXRhZGF0YSwgYW5kIHJveWFsdHkgaW50byBhIHNpbmdsZSBwZXJzaXN0ZW50IGVudHJ5LgoKQ29tYmluaW5nIHRoZXNlIGZpZWxkcyBlbGltaW5hdGVzIHRoZSBzZXBhcmF0ZSBgTWV0YWRhdGFgIGFuZCBgUm95YWx0eWAKZW50cmllcyB0aGF0IHdlcmUgcHJldmlvdXNseSB3cml0dGVuIG9uIGV2ZXJ5IG1pbnQuAAAAAAAAAAlUb2tlbkRhdGEAAAAAAAAFAAAAOFRoZSBvZmYtY2hhaW4gY2xpcCBpZGVudGlmaWVyIHRoaXMgdG9rZW4gd2FzIG1pbnRlZCBmb3IuAAAAB2NsaXBfaWQAAAAABAAAADJXaGV0aGVyIHRoaXMgdG9rZW4gaXMgc291bGJvdW5kIChub24tdHJhbnNmZXJhYmxlKQAAAAAADGlzX3NvdWxib3VuZAAAAAEAAAAaTWV0YWRhdGEgVVJJIGZvciB0aGUgdG9rZW4AAAAAAAxtZXRhZGF0YV91cmkAAAAQAAAAAAAAAAVvd25lcgAAAAAAABMAAAAVUm95YWx0eSBjb25maWd1cmF0aW9uAAAAAAAAB3JveWFsdHkAAAAH0AAAAAdSb3lhbHR5AA==",
        "AAAAAQAAADJSb3lhbHR5IHBheW1lbnQgaW5mbyByZXR1cm5lZCBieSBgcm95YWx0eV9pbmZvKClgLgAAAAAAAAAAAAtSb3lhbHR5SW5mbwAAAAADAAAAQ2BOb25lYCDihpIgcGF5IGluIFhMTTsgYFNvbWUoYWRkcilgIOKGkiBwYXkgaW4gdGhhdCBTRVAtMDA0MSB0b2tlbi4AAAAADWFzc2V0X2FkZHJlc3MAAAAAAAPoAAAAEwAAAAAAAAAIcmVjZWl2ZXIAAAATAAAAN1JveWFsdHkgYW1vdW50IGluIHRoZSBzYW1lIGRlbm9taW5hdGlvbiBhcyBgc2FsZV9wcmljZWAAAAAADnJveWFsdHlfYW1vdW50AAAAAAAL",
        "AAAAAQAAAClFdmVudCBlbWl0dGVkIHdoZW4gTkZUIG93bmVyc2hpcCBjaGFuZ2VzLgAAAAAAAAAAAAANVHJhbnNmZXJFdmVudAAAAAAAAAQAAAAAAAAABGZyb20AAAATAAAAAAAAAAhnYXNfdXNlZAAAAAYAAAAAAAAAAnRvAAAAAAATAAAAAAAAAAh0b2tlbl9pZAAAB9AAAAAHVG9rZW5JZAA=",
        "AAAAAAAAAIlCdXJuIChkZXN0cm95KSBhbiBORlQuIE9ubHkgdGhlIGN1cnJlbnQgb3duZXIgbWF5IGJ1cm4uCgpTdG9yYWdlIHJlbW92ZXMgKHBlcnNpc3RlbnQpOiBUb2tlbkRhdGEsIENsaXBJZE1pbnRlZCA9ICoqMioqIChPcHRpbWl6ZWQgZnJvbSA0KQAAAAAAAARidXJuAAAAAgAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAh0b2tlbl9pZAAAB9AAAAAHVG9rZW5JZAAAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAC5Jbml0aWFsaXplIHRoZSBjb250cmFjdCB3aXRoIGFuIGFkbWluIGFkZHJlc3MuAAAAAAAEaW5pdAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAA=",
        "AAAAAAAAA3ZNaW50IGEgbmV3IE5GVCBmb3IgYSB2aWRlbyBjbGlwLgoKUmVxdWlyZXMgYSB2YWxpZCBFZDI1NTE5IGBzaWduYXR1cmVgIGZyb20gdGhlIHJlZ2lzdGVyZWQgYmFja2VuZCBzaWduZXIKb3ZlciB0aGUgY2Fub25pY2FsIG1pbnQgcGF5bG9hZCwgcHJvdmluZyB0aGUgY2xpcCBleGlzdHMgYW5kIGJlbG9uZ3MgdG8KYHRvYC4gVGhlIHBheWxvYWQgaXM6CgpgYGB0ZXh0CnBheWxvYWQgPSBTSEEtMjU2KApjbGlwX2lkX2xlXzRfYnl0ZXMKfHwgU0hBLTI1Nihvd25lcl9hZGRyZXNzX3hkcikgICAvLyAzMiBieXRlcwp8fCBTSEEtMjU2KG1ldGFkYXRhX3VyaV9ieXRlcykgIC8vIDMyIGJ5dGVzCikKYGBgCgpTdG9yYWdlIHdyaXRlcyAocGVyc2lzdGVudCk6IFRva2VuRGF0YSwgTWV0YWRhdGEsIFJveWFsdHksIENsaXBJZE1pbnRlZCA9ICoqNCoqCkluc3RhbmNlIHdyaXRlczogTmV4dFRva2VuSWQgPSAqKjEqKgoKIyBBcmd1bWVudHMKKiBgdG9gICAgICAgICAgICAtIEFkZHJlc3MgdGhhdCB3aWxsIG93biB0aGUgTkZUIChtdXN0IG1hdGNoIHRoZSBzaWduZWQgcGF5bG9hZCkKKiBgY2xpcF9pZGAgICAgICAtIFVuaXF1ZSBvZmYtY2hhaW4gY2xpcCBpZGVudGlmaWVyIChtdXN0IG1hdGNoIHRoZSBzaWduZWQgcGF5bG9hZCkKKiBgbWV0YWRhdGFfdXJpYCAtIElQRlMgb3IgQXJ3ZWF2ZSBVUkkgKG11c3QgbWF0Y2ggdGhlIHNpZ25lZCBwYXlsb2FkKQoqIGByb3lhbHR5YCAgICAgIC0gUm95YWx0eSBjb25maWd1cmF0aW9uCiogYGlzX3NvdWxib3VuZGAgLSBXaGV0aGVyIHRoZSB0b2tlbiBpcyBzb3VsYm91bmQgKG5vbi10cmFuc2ZlcmFibGUpCiogYHNpZ25hdHVyZWAgICAgLSA2NC1ieXRlIEVkMjU1MTkgc2lnbmF0dXJlIGZyb20gdGhlIGJhY2tlbmQgc2lnbmVyAAAAAAAEbWludAAAAAYAAAAAAAAAAnRvAAAAAAATAAAAAAAAAAdjbGlwX2lkAAAAAAQAAAAAAAAADG1ldGFkYXRhX3VyaQAAABAAAAAAAAAAB3JveWFsdHkAAAAH0AAAAAdSb3lhbHR5AAAAAAAAAAAMaXNfc291bGJvdW5kAAAAAQAAAAAAAAAJc2lnbmF0dXJlAAAAAAAD7gAAAEAAAAABAAAD6QAAB9AAAAAHVG9rZW5JZAAAAAAD",
        "AAAAAAAAAFxQYXVzZSB0aGUgY29udHJhY3QuIEJsb2NrcyBgbWludGAgYW5kIGB0cmFuc2ZlcmAgdW50aWwgdW5wYXVzZWQuCk9ubHkgY2FsbGFibGUgYnkgdGhlIGFkbWluLgAAAAVwYXVzZQAAAAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAACFSZXR1cm5zIHRydWUgaWYgdGhlIHRva2VuIGV4aXN0cy4AAAAAAAAGZXhpc3RzAAAAAAABAAAAAAAAAAh0b2tlbl9pZAAAB9AAAAAHVG9rZW5JZAAAAAABAAAAAQ==",
        "AAAAAQAAACNFdmVudCBlbWl0dGVkIHdoZW4gcm95YWx0eSBpcyBwYWlkLgAAAAAAAAAAEFJveWFsdHlQYWlkRXZlbnQAAAAEAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABGZyb20AAAATAAAAAAAAAAJ0bwAAAAAAEwAAAAAAAAAIdG9rZW5faWQAAAfQAAAAB1Rva2VuSWQA",
        "AAAAAQAAAJJSb3lhbHR5IGluZm9ybWF0aW9uIHN0b3JlZCBwZXIgdG9rZW4uCmBhc3NldF9hZGRyZXNzYCBpcyBgTm9uZWAgZm9yIG5hdGl2ZSBYTE0sIG9yIGBTb21lKGNvbnRyYWN0X2FkZHJlc3MpYApmb3IgYW55IFNFUC0wMDQxIGN1c3RvbSBTdGVsbGFyIGFzc2V0LgAAAAAAAAAAABBSb3lhbHR5UmVjaXBpZW50AAAAAgAAAAAAAAAMYmFzaXNfcG9pbnRzAAAABAAAAAAAAAAJcmVjaXBpZW50AAAAAAAAEw==",
        "AAAAAAAAAFRVbnBhdXNlIHRoZSBjb250cmFjdCwgcmUtZW5hYmxpbmcgYG1pbnRgIGFuZCBgdHJhbnNmZXJgLgpPbmx5IGNhbGxhYmxlIGJ5IHRoZSBhZG1pbi4AAAAHdW5wYXVzZQAAAAABAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAB1SZXR1cm5zIHRoZSBjb250cmFjdCB2ZXJzaW9uLgAAAAAAAAd2ZXJzaW9uAAAAAAAAAAABAAAABA==",
        "AAAAAAAAACZSZXR1cm5zIHRoZSBvd25lciBvZiBhIGdpdmVuIHRva2VuIElELgAAAAAACG93bmVyX29mAAAAAQAAAAAAAAAIdG9rZW5faWQAAAfQAAAAB1Rva2VuSWQAAAAAAQAAA+kAAAATAAAAAw==",
        "AAAAAAAAAQZUcmFuc2ZlciBORlQgb3duZXJzaGlwIGZyb20gYGZyb21gIHRvIGB0b2AuCgpCbG9ja2VkIGlmIHRoZSB0b2tlbiBpcyBzb3VsYm91bmQgKG5vbi10cmFuc2ZlcmFibGUpLgpTdG9yYWdlIHdyaXRlcyAocGVyc2lzdGVudCk6IFRva2VuRGF0YSA9ICoqMSoqCgojIEFyZ3VtZW50cwoqIGBmcm9tYCAgICAgLSBDdXJyZW50IG93bmVyIChtdXN0IGF1dGhvcml6ZSkKKiBgdG9gICAgICAgIC0gTmV3IG93bmVyCiogYHRva2VuX2lkYCAtIFRva2VuIHRvIHRyYW5zZmVyAAAAAAAIdHJhbnNmZXIAAAADAAAAAAAAAARmcm9tAAAAEwAAAAAAAAACdG8AAAAAABMAAAAAAAAACHRva2VuX2lkAAAH0AAAAAdUb2tlbklkAAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAADNSZXR1cm5zIGB0cnVlYCBpZiB0aGUgY29udHJhY3QgaXMgY3VycmVudGx5IHBhdXNlZC4AAAAACWlzX3BhdXNlZAAAAAAAAAAAAAABAAAAAQ==",
        "AAAAAAAAAC5SZXR1cm5zIHRoZSBtZXRhZGF0YSBVUkkgZm9yIGEgZ2l2ZW4gdG9rZW4gSUQuAAAAAAAJdG9rZW5fdXJpAAAAAAAAAQAAAAAAAAAIdG9rZW5faWQAAAfQAAAAB1Rva2VuSWQAAAAAAQAAA+kAAAAQAAAAAw==",
        "AAAAAAAAAEJSZXR1cm4gdGhlIGN1cnJlbnRseSByZWdpc3RlcmVkIGJhY2tlbmQgc2lnbmVyIHB1YmxpYyBrZXksIGlmIGFueS4AAAAAAApnZXRfc2lnbmVyAAAAAAAAAAAAAQAAA+gAAAPuAAAAIA==",
        "AAAAAAAAAPhSZWdpc3RlciAob3Igcm90YXRlKSB0aGUgYmFja2VuZCBFZDI1NTE5IHB1YmxpYyBrZXkgdXNlZCB0byB2ZXJpZnkKY2xpcCBvd25lcnNoaXAgYmVmb3JlIG1pbnRpbmcuIE9ubHkgY2FsbGFibGUgYnkgdGhlIGFkbWluLgoKIyBBcmd1bWVudHMKKiBgYWRtaW5gICAtIE11c3QgYmUgdGhlIGNvbnRyYWN0IGFkbWluCiogYHB1YmtleWAgLSAzMi1ieXRlIEVkMjU1MTkgcHVibGljIGtleSBvZiB0aGUgdHJ1c3RlZCBiYWNrZW5kIHNpZ25lcgAAAApzZXRfc2lnbmVyAAAAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAABnB1YmtleQAAAAAD7gAAACAAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAADBSZXR1cm5zIHRoZSBzdG9yZWQgYFJveWFsdHlgIHN0cnVjdCBmb3IgYSB0b2tlbi4AAAALZ2V0X3JveWFsdHkAAAAAAQAAAAAAAAAIdG9rZW5faWQAAAfQAAAAB1Rva2VuSWQAAAAAAQAAA+kAAAfQAAAAB1JveWFsdHkAAAAAAw==",
        "AAAAAAAAAMZQYXkgcm95YWx0aWVzIGZvciBhIHRva2VuIHNhbGUgdXNpbmcgdGhlIGFzc2V0IGNvbmZpZ3VyZWQgaW4gdGhlIHJveWFsdHkuCgpPbmx5IGhhbmRsZXMgU0VQLTAwNDEgY3VzdG9tIGFzc2V0cy4gRm9yIFhMTSAoYGFzc2V0X2FkZHJlc3NgIGlzIGBOb25lYCkKdGhlIG1hcmtldHBsYWNlIG11c3QgaGFuZGxlIHRoZSB0cmFuc2ZlciBkaXJlY3RseS4AAAAAAAtwYXlfcm95YWx0eQAAAAADAAAAAAAAAAVwYXllcgAAAAAAABMAAAAAAAAACHRva2VuX2lkAAAH0AAAAAdUb2tlbklkAAAAAAAAAAAKc2FsZV9wcmljZQAAAAAACwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAIFVcGRhdGUgdGhlIHJveWFsdHkgY29uZmlndXJhdGlvbiBmb3IgYSB0b2tlbi4gQWRtaW4gb25seS4KRW1pdHMgUm95YWx0eVJlY2lwaWVudFVwZGF0ZWQgZXZlbnQgd2hlbiB0aGUgcHJpbWFyeSByZWNpcGllbnQgY2hhbmdlcy4AAAAAAAALc2V0X3JveWFsdHkAAAAAAwAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAh0b2tlbl9pZAAAB9AAAAAHVG9rZW5JZAAAAAAAAAAAC25ld19yb3lhbHR5AAAAB9AAAAAHUm95YWx0eQAAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAAC5BbGlhcyBmb3IgYHRva2VuX3VyaWAsIGtlcHQgZm9yIGNvbXBhdGliaWxpdHkuAAAAAAAMZ2V0X21ldGFkYXRhAAAAAQAAAAAAAAAIdG9rZW5faWQAAAfQAAAAB1Rva2VuSWQAAAAAAQAAA+kAAAAQAAAAAw==",
        "AAAAAAAAADpSZXR1cm5zIHRydWUgaWYgdGhlIHRva2VuIGlzIHNvdWxib3VuZCAobm9uLXRyYW5zZmVyYWJsZSkuAAAAAAAMaXNfc291bGJvdW5kAAAAAQAAAAAAAAAIdG9rZW5faWQAAAfQAAAAB1Rva2VuSWQAAAAAAQAAAAE=",
        "AAAAAAAAARpSZXR1cm5zIHRoZSByb3lhbHR5IHJlY2VpdmVyLCBhbW91bnQsIGFuZCBwYXltZW50IGFzc2V0IGZvciBhIGdpdmVuIHNhbGUgcHJpY2UuCgpVc2VzIHNhZmUgbWF0aCB0byBwcmV2ZW50IG92ZXJmbG93LiBSb3lhbHR5IGFtb3VudCBpcyBjYWxjdWxhdGVkIGFzOgpgcm95YWx0eV9hbW91bnQgPSBzYWxlX3ByaWNlICogYmFzaXNfcG9pbnRzIC8gMTAwMDBgCgpTYWZlIGxpbWl0czogc2FsZV9wcmljZSBzaG91bGQgbm90IGV4Y2VlZCBpMTI4OjpNQVggLyAxMDAwMCB0byBhdm9pZCBvdmVyZmxvdy4AAAAAAAxyb3lhbHR5X2luZm8AAAACAAAAAAAAAAh0b2tlbl9pZAAAB9AAAAAHVG9rZW5JZAAAAAAAAAAACnNhbGVfcHJpY2UAAAAAAAsAAAABAAAD6QAAB9AAAAALUm95YWx0eUluZm8AAAAAAw==",
        "AAAAAAAAAH9SZXR1cm5zIHRoZSB0b3RhbCBudW1iZXIgb2YgbWludGVkIChhbmQgbm90IHlldCBidXJuZWQpIHRva2Vucy4KCkRlcml2ZWQgZnJvbSBgTmV4dFRva2VuSWQgLSAxYCDigJQgbm8gc2VwYXJhdGUgY291bnRlciBuZWVkZWQuAAAAAAx0b3RhbF9zdXBwbHkAAAAAAAAAAQAAAAQ=",
        "AAAAAAAAADJMb29rIHVwIHRoZSBvbi1jaGFpbiB0b2tlbiBJRCBmb3IgYSBnaXZlbiBjbGlwX2lkLgAAAAAADWNsaXBfdG9rZW5faWQAAAAAAAABAAAAAAAAAAdjbGlwX2lkAAAAAAQAAAABAAAD6QAAB9AAAAAHVG9rZW5JZAAAAAAD",
        "AAAAAAAAAFlSZXR1cm5zIHRoZSBhdmVyYWdlIHN5bnRoZXRpYyBnYXMgY29zdCBmb3IgYSBnaXZlbiBvcGVyYXRpb24gdHlwZS4KMCA9IE1pbnQsIDEgPSBUcmFuc2ZlcgAAAAAAABBnZXRfYXZnX2dhc19jb3N0AAAAAQAAAAAAAAAHb3BfdHlwZQAAAAAEAAAAAQAAAAY=",
        "AAAAAQAAADBFdmVudCBlbWl0dGVkIHdoZW4gcm95YWx0eSByZWNpcGllbnQgaXMgdXBkYXRlZC4AAAAAAAAAHFJveWFsdHlSZWNpcGllbnRVcGRhdGVkRXZlbnQAAAADAAAAAAAAAA1uZXdfcmVjaXBpZW50AAAAAAAAEwAAAAAAAAANb2xkX3JlY2lwaWVudAAAAAAAABMAAAAAAAAACHRva2VuX2lkAAAH0AAAAAdUb2tlbklkAA==" ]),
      options
    )
  }
  public readonly fromJSON = {
    burn: this.txFromJSON<Result<void>>,
        init: this.txFromJSON<null>,
        mint: this.txFromJSON<Result<TokenId>>,
        pause: this.txFromJSON<Result<void>>,
        exists: this.txFromJSON<boolean>,
        unpause: this.txFromJSON<Result<void>>,
        version: this.txFromJSON<u32>,
        owner_of: this.txFromJSON<Result<string>>,
        transfer: this.txFromJSON<Result<void>>,
        is_paused: this.txFromJSON<boolean>,
        token_uri: this.txFromJSON<Result<string>>,
        get_signer: this.txFromJSON<Option<Buffer>>,
        set_signer: this.txFromJSON<Result<void>>,
        get_royalty: this.txFromJSON<Result<Royalty>>,
        pay_royalty: this.txFromJSON<Result<void>>,
        set_royalty: this.txFromJSON<Result<void>>,
        get_metadata: this.txFromJSON<Result<string>>,
        is_soulbound: this.txFromJSON<boolean>,
        royalty_info: this.txFromJSON<Result<RoyaltyInfo>>,
        total_supply: this.txFromJSON<u32>,
        clip_token_id: this.txFromJSON<Result<TokenId>>,
        get_avg_gas_cost: this.txFromJSON<u64>,
        blacklist_clip: this.txFromJSON<Result<void>>,
        update_royalty_recipient: this.txFromJSON<Result<void>>,
        tokens_of_owner: this.txFromJSON<Array<TokenId>>,
        calculate_royalty_amount: this.txFromJSON<Result<i128>>,
        batch_mint: this.txFromJSON<Result<Array<TokenId>>>
  }
}