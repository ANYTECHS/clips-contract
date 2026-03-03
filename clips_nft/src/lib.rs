//! ClipCash NFT - Soroban Smart Contract
//! 
//! This contract enables minting video clips as NFTs on the Stellar network
//! with built-in royalty support for content creators.

#![no_std]

use soroban_sdk::{
    auth::Context,
    contract, contracterror, contractimpl, contracttype,
    address::Address,
    collections::Map,
    env,
    symbol_short,
    vec,
    Symbol,
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

/// Token ID type
pub type TokenId = u64;

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

/// NFT Contract
#[contract]
pub struct ClipsNftContract;

#[contractimpl]
impl ClipsNftContract {
    /// Initialize the contract with admin
    pub fn init(env: soroban_sdk::Env, admin: Address) {
        // Store the admin
        let admin_key = Symbol::new(&env, "admin");
        env.storage().instance().set(&admin_key, &admin);
        
        // Initialize token count
        let token_count_key = Symbol::new(&env, "token_count");
        env.storage().instance().set(&token_count_key, &0u64);
        
        // Initialize next token ID
        let next_token_key = Symbol::new(&env, "next_token_id");
        env.storage().instance().set(&next_token_key, &1u64);
    }

    /// Mint a new NFT for a video clip
    /// 
    /// # Arguments
    /// * `to` - Address that will own the NFT
    /// * `metadata` - Token metadata
    /// * `royalty` - Royalty configuration
    /// 
    /// # Returns
    /// The token ID of the newly minted NFT
    pub fn mint(
        env: soroban_sdk::Env,
        admin: Address,
        to: Address,
        metadata: TokenMetadata,
        royalty: Royalty,
    ) -> Result<TokenId, Error> {
        // Verify admin is authorized
        self._require_admin(&env, &admin)?;
        
        // Validate royalty (max 100%)
        if royalty.basis_points > 10000 {
            return Err(Error::RoyaltyTooHigh);
        }
        
        // Get next token ID
        let next_token_key = Symbol::new(&env, "next_token_id");
        let token_id: u64 = env.storage().instance().get(&next_token_key)
            .unwrap_or(1);
        
        // Store token metadata
        let metadata_key = (Symbol::new(&env, "metadata"), token_id);
        env.storage().persistent().set(&metadata_key, &metadata);
        
        // Store royalty
        let royalty_key = (Symbol::new(&env, "royalty"), token_id);
        env.storage().persistent().set(&royalty_key, &royalty);
        
        // Store owner
        let owner_key = (Symbol::new(&env, "owner"), token_id);
        env.storage().persistent().set(&owner_key, &to);
        
        // Increment next token ID
        env.storage().instance().set(&next_token_key, &(token_id + 1));
        
        // Update token count
        let token_count_key = Symbol::new(&env, "token_count");
        let count: u64 = env.storage().instance().get(&token_count_key).unwrap_or(0);
        env.storage().instance().set(&token_count_key, &(count + 1));
        
        // Emit event
        soroban_sdk::log!(&env, "NFT minted: {} to {}", token_id, to);
        
        Ok(token_id)
    }

    /// Transfer NFT ownership
    pub fn transfer(
        env: soroban_sdk::Env,
        from: Address,
        to: Address,
        token_id: TokenId,
    ) -> Result<(), Error> {
        // Verify owner
        let owner_key = (Symbol::new(&env, "owner"), token_id);
        let owner: Address = env.storage().persistent().get(&owner_key)
            .ok_or(Error::InvalidTokenId)?;
        
        // Verify caller is owner or authorized
        if from != owner {
            return Err(Error::Unauthorized);
        }
        
        // Update owner
        env.storage().persistent().set(&owner_key, &to);
        
        soroban_sdk::log!(&env, "NFT transferred: {} from {} to {}", token_id, from, to);
        
        Ok(())
    }

    /// Get token metadata
    pub fn get_metadata(env: soroban_sdk::Env, token_id: TokenId) -> Result<TokenMetadata, Error> {
        let metadata_key = (Symbol::new(&env, "metadata"), token_id);
        env.storage().persistent().get(&metadata_key)
            .ok_or(Error::InvalidTokenId)
    }

    /// Get royalty info
    pub fn get_royalty(env: soroban_sdk::Env, token_id: TokenId) -> Result<Royalty, Error> {
        let royalty_key = (Symbol::new(&env, "royalty"), token_id);
        env.storage().persistent().get(&royalty_key)
            .ok_or(Error::InvalidTokenId)
    }

    /// Get token owner
    pub fn get_owner(env: soroban_sdk::Env, token_id: TokenId) -> Result<Address, Error> {
        let owner_key = (Symbol::new(&env, "owner"), token_id);
        env.storage().persistent().get(&owner_key)
            .ok_or(Error::InvalidTokenId)
    }

    /// Get total supply
    pub fn total_supply(env: soroban_sdk::Env) -> u64 {
        let token_count_key = Symbol::new(&env, "token_count");
        env.storage().instance().get(&token_count_key).unwrap_or(0)
    }

    /// Check if token exists
    pub fn exists(env: soroban_sdk::Env, token_id: TokenId) -> bool {
        let owner_key = (Symbol::new(&env, "owner"), token_id);
        env.storage().persistent().contains(&owner_key)
    }

    /// Burn (destroy) an NFT
    pub fn burn(env: soroban_sdk::Env, owner: Address, token_id: TokenId) -> Result<(), Error> {
        // Verify owner
        let owner_key = (Symbol::new(&env, "owner"), token_id);
        let current_owner: Address = env.storage().persistent().get(&owner_key)
            .ok_or(Error::InvalidTokenId)?;
        
        if owner != current_owner {
            return Err(Error::Unauthorized);
        }
        
        // Remove all data
        env.storage().persistent().remove(&owner_key);
        
        let metadata_key = (Symbol::new(&env, "metadata"), token_id);
        env.storage().persistent().remove(&metadata_key);
        
        let royalty_key = (Symbol::new(&env, "royalty"), token_id);
        env.storage().persistent().remove(&royalty_key);
        
        // Update count
        let token_count_key = Symbol::new(&env, "token_count");
        let count: u64 = env.storage().instance().get(&token_count_key).unwrap_or(0);
        env.storage().instance().set(&token_count_key, &(count.saturating_sub(1)));
        
        Ok(())
    }

    /// Internal function to verify admin
    fn _require_admin(env: &soroban_sdk::Env, addr: &Address) -> Result<(), Error> {
        let admin_key = Symbol::new(env, "admin");
        let admin: Address = env.storage().instance().get(&admin_key)
            .expect("Admin not initialized");
        
        if addr != &admin {
            return Err(Error::Unauthorized);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_and_mint() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        // Initialize contract
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        
        client.init(&admin);
        
        // Create metadata
        let metadata = TokenMetadata {
            title: "My First Clip".to_string(),
            description: "A viral moment".to_string(),
            media_url: "ipfs://QmExample".to_string(),
            thumbnail_url: "ipfs://QmThumb".to_string(),
            creator: user.clone(),
            created_at: 1000,
        };
        
        let royalty = Royalty {
            recipient: user.clone(),
            basis_points: 500, // 5%
        };
        
        // Mint NFT
        let token_id = client.mint(&admin, &user, &metadata, &royalty);
        assert_eq!(token_id, 1);
        
        // Verify metadata
        let retrieved_metadata = client.get_metadata(&token_id);
        assert_eq!(retrieved_metadata.title, "My First Clip");
        
        // Verify royalty
        let retrieved_royalty = client.get_royalty(&token_id);
        assert_eq!(retrieved_royalty.basis_points, 500);
        
        // Verify owner
        let owner = client.get_owner(&token_id);
        assert_eq!(owner, user);
        
        // Verify total supply
        let supply = client.total_supply();
        assert_eq!(supply, 1);
    }

    #[test]
    fn test_transfer() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        
        client.init(&admin);
        
        // Mint NFT to user1
        let metadata = TokenMetadata {
            title: "Test".to_string(),
            description: "Test".to_string(),
            media_url: "".to_string(),
            thumbnail_url: "".to_string(),
            creator: user1.clone(),
            created_at: 1000,
        };
        
        let royalty = Royalty {
            recipient: user1.clone(),
            basis_points: 500,
        };
        
        let token_id = client.mint(&admin, &user1, &metadata, &royalty);
        
        // Transfer to user2
        client.transfer(&user1, &user2, &token_id);
        
        // Verify new owner
        let owner = client.get_owner(&token_id);
        assert_eq!(owner, user2);
    }

    #[test]
    fn test_burn() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        
        client.init(&admin);
        
        // Mint NFT
        let metadata = TokenMetadata {
            title: "Test".to_string(),
            description: "Test".to_string(),
            media_url: "".to_string(),
            thumbnail_url: "".to_string(),
            creator: user.clone(),
            created_at: 1000,
        };
        
        let royalty = Royalty {
            recipient: user.clone(),
            basis_points: 500,
        };
        
        let token_id = client.mint(&admin, &user, &metadata, &royalty);
        
        // Burn NFT
        client.burn(&user, &token_id);
        
        // Verify burned (owner should not exist)
        let exists = client.exists(&token_id);
        assert!(!exists);
    }
}
