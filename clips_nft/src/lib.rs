//! ClipCash NFT - Soroban Smart Contract
//!
//! This contract enables minting video clips as NFTs on the Stellar network
//! with built-in royalty support for content creators.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    Address, Env, String, Symbol,
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

/// NFT metadata
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenMetadata {
    /// Title of the clip
    pub title: String,
    /// Description of the clip
    pub description: String,
    /// IPFS or HTTP URL to the clip media
    pub media_url: String,
    /// IPFS or HTTP URL to the thumbnail
    pub thumbnail_url: String,
    /// Creator/owner of the clip
    pub creator: Address,
    /// Timestamp when created
    pub created_at: u64,
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

    /// Mint a new NFT for a video clip
    pub fn mint(
        env: Env,
        admin: Address,
        to: Address,
        metadata: TokenMetadata,
        royalty: Royalty,
    ) -> Result<TokenId, Error> {
        Self::require_admin(&env, &admin)?;

        if royalty.basis_points > 10000 {
            return Err(Error::RoyaltyTooHigh);
        }

        let token_id: TokenId = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(1);

        env.storage().persistent().set(&DataKey::Metadata(token_id), &metadata);
        env.storage().persistent().set(&DataKey::Royalty(token_id), &royalty);
        env.storage().persistent().set(&DataKey::Owner(token_id), &to);

        // Increment next token ID
        env.storage()
            .instance()
            .set(&DataKey::NextTokenId, &(token_id + 1));

        // Update total token count
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
            .set(&DataKey::Balance(to), &(balance + 1));

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

    /// Returns the token URI (media_url from metadata) for a given token ID
    pub fn token_uri(env: Env, token_id: TokenId) -> Result<String, Error> {
        let metadata: TokenMetadata = env
            .storage()
            .persistent()
            .get(&DataKey::Metadata(token_id))
            .ok_or(Error::InvalidTokenId)?;
        Ok(metadata.media_url)
    }

    // -------------------------------------------------------------------------
    // Additional view helpers
    // -------------------------------------------------------------------------

    /// Get full token metadata
    pub fn get_metadata(env: Env, token_id: TokenId) -> Result<TokenMetadata, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Metadata(token_id))
            .ok_or(Error::InvalidTokenId)
    }

    /// Get royalty info
    pub fn get_royalty(env: Env, token_id: TokenId) -> Result<Royalty, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Royalty(token_id))
            .ok_or(Error::InvalidTokenId)
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

    fn default_metadata(env: &Env, creator: Address) -> TokenMetadata {
        TokenMetadata {
            title: String::from_str(env, "My Clip"),
            description: String::from_str(env, "A viral moment"),
            media_url: String::from_str(env, "ipfs://QmExample"),
            thumbnail_url: String::from_str(env, "ipfs://QmThumb"),
            creator,
            created_at: 1000,
        }
    }

    fn default_royalty(env: &Env, recipient: Address) -> Royalty {
        let _ = env;
        Royalty { recipient, basis_points: 500 }
    }

    #[test]
    fn test_init_and_mint() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);

        client.init(&admin);

        let token_id = client.mint(
            &admin,
            &user1,
            &default_metadata(&env, user1.clone()),
            &default_royalty(&env, user1.clone()),
        );
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
    fn test_transfer_updates_balances() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);

        client.init(&admin);
        let token_id = client.mint(
            &admin,
            &user1,
            &default_metadata(&env, user1.clone()),
            &default_royalty(&env, user1.clone()),
        );

        client.transfer(&user1, &user2, &token_id);

        assert_eq!(client.owner_of(&token_id), user2);
        assert_eq!(client.balance_of(&user1), 0);
        assert_eq!(client.balance_of(&user2), 1);
    }

    #[test]
    fn test_burn() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);

        client.init(&admin);
        let token_id = client.mint(
            &admin,
            &user1,
            &default_metadata(&env, user1.clone()),
            &default_royalty(&env, user1.clone()),
        );

        client.burn(&user1, &token_id);

        assert!(!client.exists(&token_id));
        assert_eq!(client.balance_of(&user1), 0);
        assert_eq!(client.total_supply(), 0);
    }
}
