use soroban_sdk::{contracttype, Address, Env, String, Vec};

use crate::metadata::Attribute;
use crate::types::{Royalty, TokenId};

/// A complete NFT mint object that contains all data needed to create an NFT
/// before persistence.
#[contracttype]
#[derive(Clone, Debug)]
pub struct NftMint {
    pub token_id: TokenId,
    pub owner: Address,
    pub clip_id: u32,
    pub metadata_uri: String,
    pub image: Option<String>,
    pub animation_url: Option<String>,
    pub description: Option<String>,
    pub external_url: Option<String>,
    pub attributes: Vec<Attribute>,
    pub royalty: Royalty,
    pub is_soulbound: bool,
    pub is_locked: bool,
    pub creator: Address,
    pub collection: String,
    pub mint_timestamp: u64,
    pub labels: Vec<String>,
}

/// Factory for creating NFT mint objects
pub struct NftMintFactory;

impl NftMintFactory {
    /// Create a new NFT mint object with all required fields
    pub fn create(
        env: &Env,
        token_id: TokenId,
        owner: Address,
        clip_id: u32,
        metadata_uri: String,
        royalty: Royalty,
        creator: Address,
        collection: String,
    ) -> NftMint {
        NftMint {
            token_id,
            owner,
            clip_id,
            metadata_uri,
            image: None,
            animation_url: None,
            description: None,
            external_url: None,
            attributes: Vec::new(env),
            royalty,
            is_soulbound: false,
            is_locked: false,
            creator,
            collection,
            mint_timestamp: env.ledger().timestamp(),
            labels: Vec::new(env),
        }
    }

    /// Create an NFT mint object with optional fields
    pub fn create_with_options(
        env: &Env,
        token_id: TokenId,
        owner: Address,
        clip_id: u32,
        metadata_uri: String,
        royalty: Royalty,
        creator: Address,
        collection: String,
        image: Option<String>,
        animation_url: Option<String>,
        description: Option<String>,
        external_url: Option<String>,
        attributes: Vec<Attribute>,
        is_soulbound: bool,
        labels: Vec<String>,
    ) -> NftMint {
        NftMint {
            token_id,
            owner,
            clip_id,
            metadata_uri,
            image,
            animation_url,
            description,
            external_url,
            attributes,
            royalty,
            is_soulbound,
            is_locked: false,
            creator,
            collection,
            mint_timestamp: env.ledger().timestamp(),
            labels,
        }
    }
}
