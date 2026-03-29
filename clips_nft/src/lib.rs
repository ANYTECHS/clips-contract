//! ClipCash NFT - Soroban Smart Contract
//!
//! This contract enables minting video clips as NFTs on the Stellar network
//! with built-in royalty support for content creators.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    symbol_short, Address, Env, String,
};

/// Custom errors for the NFT contract
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Error {
    /// Operation not authorized
    Unauthorized = 1,
    /// Invalid token ID
    InvalidTokenId = 2,
    /// Token already minted
    TokenAlreadyMinted = 3,
    /// Royalty too high (max 10000 basis points = 100%)
    RoyaltyTooHigh = 4,
    /// Invalid recipient
    InvalidRecipient = 5,
    /// Sale price must be greater than zero
    InvalidSalePrice = 6,
}

/// Token ID type (u32 for ERC721-like compatibility)
pub type TokenId = u32;

/// Royalty information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Royalty {
    /// Recipient address who receives royalties
    pub recipient: Address,
    /// Royalty amount in basis points (0-10000, where 10000 = 100%)
    pub basis_points: u32,
}

/// Royalty payment info returned by royalty_info()
/// Follows the EIP-2981 / Soroban royalty extension pattern:
/// given a sale price, returns who to pay and how much.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyInfo {
    /// Address that should receive the royalty payment
    pub receiver: Address,
    /// Royalty amount in the same denomination as sale_price
    pub royalty_amount: i128,
}

/// Storage keys
#[contracttype]
pub enum DataKey {
    Admin,
    TokenCount,
    NextTokenId,
    Owner(TokenId),
    Metadata(TokenId),
    Royalty(TokenId),
    Balance(Address),
    /// Maps clip_id -> token_id; used to prevent double-minting
    ClipIdMinted(u32),
    /// Maps token_id -> clip_id; used for burn cleanup
    TokenClipId(TokenId),
}

/// Event emitted when a new NFT is minted
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintEvent {
    /// Recipient of the newly minted token
    pub to: Address,
    /// The clip ID that was minted
    pub clip_id: u32,
    /// The assigned on-chain token ID
    pub token_id: TokenId,
    /// Metadata URI (IPFS / Arweave)
    pub metadata_uri: String,
}

/// NFT Contract
#[contract]
pub struct ClipsNftContract;

#[contractimpl]
impl ClipsNftContract {
    /// Initialize the contract with admin
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenCount, &0u32);
        env.storage().instance().set(&DataKey::NextTokenId, &1u32);
    }

    /// Mint a new NFT for a video clip.
    ///
    /// Follows the NonFungibleToken mint pattern (SEP-0041 style):
    /// the caller must be the contract admin. Each `clip_id` can only
    /// be minted once — attempting to re-mint the same clip returns
    /// `TokenAlreadyMinted`.
    ///
    /// # Arguments
    /// * `to`           - Address that will own the NFT
    /// * `clip_id`      - Unique off-chain clip identifier (u32)
    /// * `metadata_uri` - IPFS or Arweave URI pointing to the clip metadata JSON
    /// * `royalty`      - Royalty configuration for this token
    ///
    /// # Returns
    /// The on-chain `TokenId` assigned to this clip.
    ///
    /// # Events
    /// Emits a `(symbol_short!("mint"), MintEvent)` event on success.
    pub fn mint(
        env: Env,
        admin: Address,
        to: Address,
        clip_id: u32,
        metadata_uri: String,
        royalty: Royalty,
    ) -> Result<TokenId, Error> {
        Self::require_admin(&env, &admin)?;

        // Prevent double-minting the same clip
        if env
            .storage()
            .persistent()
            .contains_key(&DataKey::ClipIdMinted(clip_id))
        {
            return Err(Error::TokenAlreadyMinted);
        }

        if royalty.basis_points > 10000 {
            return Err(Error::RoyaltyTooHigh);
        }

        let token_id: TokenId = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(1);

        // Store metadata URI directly (lightweight — full JSON lives off-chain)
        env.storage()
            .persistent()
            .set(&DataKey::Metadata(token_id), &metadata_uri);
        env.storage()
            .persistent()
            .set(&DataKey::Royalty(token_id), &royalty);
        env.storage()
            .persistent()
            .set(&DataKey::Owner(token_id), &to);

        // Mark clip_id as minted → token_id (and reverse for burn cleanup)
        env.storage()
            .persistent()
            .set(&DataKey::ClipIdMinted(clip_id), &token_id);
        env.storage()
            .persistent()
            .set(&DataKey::TokenClipId(token_id), &clip_id);

        // Increment counters
        env.storage()
            .instance()
            .set(&DataKey::NextTokenId, &(token_id + 1));

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TokenCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TokenCount, &(count + 1));

        // Update owner balance
        let balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(balance + 1));

        // Emit Mint event
        env.events().publish(
            (symbol_short!("mint"),),
            MintEvent {
                to,
                clip_id,
                token_id,
                metadata_uri,
            },
        );

        Ok(token_id)
    }

    /// Transfer NFT ownership
    pub fn transfer(env: Env, from: Address, to: Address, token_id: TokenId) -> Result<(), Error> {
        from.require_auth();

        let owner: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Owner(token_id))
            .ok_or(Error::InvalidTokenId)?;

        if from != owner {
            return Err(Error::Unauthorized);
        }

        env.storage().persistent().set(&DataKey::Owner(token_id), &to);

        // Update balances
        let from_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from), &from_balance.saturating_sub(1));

        let to_balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to), &(to_balance + 1));

        Ok(())
    }

    // -------------------------------------------------------------------------
    // ERC721-like standard view functions
    // -------------------------------------------------------------------------

    /// Returns the owner of a given token ID
    pub fn owner_of(env: Env, token_id: TokenId) -> Result<Address, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Owner(token_id))
            .ok_or(Error::InvalidTokenId)
    }

    /// Returns the number of tokens owned by an address
    pub fn balance_of(env: Env, owner: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(owner))
            .unwrap_or(0)
    }

    /// Returns the token URI (metadata_uri) for a given token ID
    pub fn token_uri(env: Env, token_id: TokenId) -> Result<String, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Metadata(token_id))
            .ok_or(Error::InvalidTokenId)
    }

    // -------------------------------------------------------------------------
    // Additional view helpers
    // -------------------------------------------------------------------------

    /// Get the metadata URI for a token (same as token_uri, kept for compatibility)
    pub fn get_metadata(env: Env, token_id: TokenId) -> Result<String, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Metadata(token_id))
            .ok_or(Error::InvalidTokenId)
    }

    /// Look up the on-chain token ID for a given clip_id.
    /// Returns `InvalidTokenId` if the clip has not been minted yet.
    pub fn clip_token_id(env: Env, clip_id: u32) -> Result<TokenId, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::ClipIdMinted(clip_id))
            .ok_or(Error::InvalidTokenId)
    }

    /// Get royalty info
    pub fn get_royalty(env: Env, token_id: TokenId) -> Result<Royalty, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Royalty(token_id))
            .ok_or(Error::InvalidTokenId)
    }

    // -------------------------------------------------------------------------
    // Royalty extension (EIP-2981 style)
    // -------------------------------------------------------------------------

    /// Returns the royalty receiver and amount for a given sale price.
    ///
    /// This is the standard royalty query used by marketplaces:
    ///   royalty_amount = sale_price * basis_points / 10000
    ///
    /// # Arguments
    /// * `token_id`   - The token being sold
    /// * `sale_price` - The gross sale price in the payment token's smallest unit
    pub fn royalty_info(
        env: Env,
        token_id: TokenId,
        sale_price: i128,
    ) -> Result<RoyaltyInfo, Error> {
        if sale_price <= 0 {
            return Err(Error::InvalidSalePrice);
        }

        let royalty: Royalty = env
            .storage()
            .persistent()
            .get(&DataKey::Royalty(token_id))
            .ok_or(Error::InvalidTokenId)?;

        // basis_points / 10000 * sale_price, using integer arithmetic to avoid
        // floating point. Soroban has no f64, so we do: amount = price * bp / 10000
        let royalty_amount = sale_price
            .saturating_mul(royalty.basis_points as i128)
            / 10_000;

        Ok(RoyaltyInfo {
            receiver: royalty.recipient,
            royalty_amount,
        })
    }

    /// Update the royalty configuration for a token.
    /// Only callable by the contract admin.
    ///
    /// # Arguments
    /// * `admin`      - Must be the contract admin
    /// * `token_id`   - Token whose royalty is being updated
    /// * `new_royalty` - New royalty configuration
    pub fn set_royalty(
        env: Env,
        admin: Address,
        token_id: TokenId,
        new_royalty: Royalty,
    ) -> Result<(), Error> {
        Self::require_admin(&env, &admin)?;

        // Token must exist
        if !env.storage().persistent().contains_key(&DataKey::Owner(token_id)) {
            return Err(Error::InvalidTokenId);
        }

        if new_royalty.basis_points > 10000 {
            return Err(Error::RoyaltyTooHigh);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Royalty(token_id), &new_royalty);

        Ok(())
    }

    /// Get total supply
    pub fn total_supply(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::TokenCount)
            .unwrap_or(0)
    }

    /// Check if token exists
    pub fn exists(env: Env, token_id: TokenId) -> bool {
        env.storage().persistent().contains_key(&DataKey::Owner(token_id))
    }

    /// Burn (destroy) an NFT
    pub fn burn(env: Env, owner: Address, token_id: TokenId) -> Result<(), Error> {
        owner.require_auth();

        let current_owner: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Owner(token_id))
            .ok_or(Error::InvalidTokenId)?;

        if owner != current_owner {
            return Err(Error::Unauthorized);
        }

        env.storage().persistent().remove(&DataKey::Owner(token_id));
        env.storage().persistent().remove(&DataKey::Metadata(token_id));
        env.storage().persistent().remove(&DataKey::Royalty(token_id));

        // Clean up clip_id mappings so the clip can be re-minted if desired
        if let Some(clip_id) = env
            .storage()
            .persistent()
            .get::<DataKey, u32>(&DataKey::TokenClipId(token_id))
        {
            env.storage()
                .persistent()
                .remove(&DataKey::ClipIdMinted(clip_id));
            env.storage()
                .persistent()
                .remove(&DataKey::TokenClipId(token_id));
        }

        // Update total count
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TokenCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TokenCount, &count.saturating_sub(1));

        // Update owner balance
        let balance: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(owner.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(owner), &balance.saturating_sub(1));

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    fn require_admin(env: &Env, addr: &Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not initialized");

        if addr != &admin {
            return Err(Error::Unauthorized);
        }

        addr.require_auth();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        (env, admin, user1, user2)
    }

    fn default_royalty(recipient: Address) -> Royalty {
        Royalty { recipient, basis_points: 500 }
    }

    /// Helper: mint clip 1 to user1 with a standard URI
    fn do_mint(
        client: &ClipsNftContractClient,
        env: &Env,
        admin: &Address,
        to: &Address,
        clip_id: u32,
    ) -> TokenId {
        client.mint(
            admin,
            to,
            &clip_id,
            &String::from_str(env, "ipfs://QmExample"),
            &default_royalty(to.clone()),
        )
    }

    #[test]
    fn test_mint_stores_owner_and_uri() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let token_id = do_mint(&client, &env, &admin, &user1, 42);
        assert_eq!(token_id, 1);

        assert_eq!(client.owner_of(&token_id), user1);
        assert_eq!(client.balance_of(&user1), 1);
        assert_eq!(
            client.token_uri(&token_id),
            String::from_str(&env, "ipfs://QmExample")
        );
        assert_eq!(client.total_supply(), 1);
    }

    #[test]
    fn test_clip_token_id_lookup() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let token_id = do_mint(&client, &env, &admin, &user1, 99);
        assert_eq!(client.clip_token_id(&99), token_id);
    }

    #[test]
    #[should_panic]
    fn test_double_mint_same_clip_id_panics() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        do_mint(&client, &env, &admin, &user1, 7);
        // Second mint with same clip_id must fail
        do_mint(&client, &env, &admin, &user1, 7);
    }

    #[test]
    fn test_mint_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let token_id = do_mint(&client, &env, &admin, &user1, 5);

        let events = env.events().all();
        // The last event should be our mint event
        assert!(!events.is_empty());
        let (_, _, event_data): (_, soroban_sdk::Vec<soroban_sdk::Val>, MintEvent) =
            events.last().unwrap();
        assert_eq!(event_data.clip_id, 5);
        assert_eq!(event_data.token_id, token_id);
        assert_eq!(event_data.to, user1);
    }

    #[test]
    fn test_transfer_updates_balances() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let token_id = do_mint(&client, &env, &admin, &user1, 1);
        client.transfer(&user1, &user2, &token_id);

        assert_eq!(client.owner_of(&token_id), user2);
        assert_eq!(client.balance_of(&user1), 0);
        assert_eq!(client.balance_of(&user2), 1);
    }

    #[test]
    fn test_royalty_info() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let token_id = do_mint(&client, &env, &admin, &user1, 1); // 500 bp = 5%

        // 5% of 1_000_000 = 50_000
        let info = client.royalty_info(&token_id, &1_000_000i128);
        assert_eq!(info.receiver, user1);
        assert_eq!(info.royalty_amount, 50_000i128);

        let zero_royalty = Royalty { recipient: user1.clone(), basis_points: 0 };
        client.set_royalty(&admin, &token_id, &zero_royalty);
        let info2 = client.royalty_info(&token_id, &1_000_000i128);
        assert_eq!(info2.royalty_amount, 0i128);
    }

    #[test]
    fn test_set_royalty() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let token_id = do_mint(&client, &env, &admin, &user1, 1);

        let new_royalty = Royalty { recipient: user2.clone(), basis_points: 1000 };
        client.set_royalty(&admin, &token_id, &new_royalty);

        let stored = client.get_royalty(&token_id);
        assert_eq!(stored.recipient, user2);
        assert_eq!(stored.basis_points, 1000);

        let info = client.royalty_info(&token_id, &500i128);
        assert_eq!(info.receiver, user2);
        assert_eq!(info.royalty_amount, 50i128);
    }

    #[test]
    fn test_burn() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);

        let token_id = do_mint(&client, &env, &admin, &user1, 1);
        client.burn(&user1, &token_id);

        assert!(!client.exists(&token_id));
        assert_eq!(client.balance_of(&user1), 0);
        assert_eq!(client.total_supply(), 0);
    }
}
