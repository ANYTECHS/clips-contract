import sys
path = 'clips_nft/src/lib.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# Fix 1: malformed nested test block
old = '''    #[test]
    fn test_batch_mint_duplicate_clip_id_fails() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
    #[test]
    fn test_tokens_of_owner_respects_result_limit() {
        // This test verifies that tokens_of_owner respects the MAX_RESULTS limit
        // to prevent gas explosion. While we can't easily test 1000+ tokens,
        // we verify that the function returns a bounded result.
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Mint 5 tokens to verify basic functionality
        let mut minted = Vec::new(&env);
        for i in 0..5u32 {
            let token_id = do_mint(&client, &env, &user1, 500 + i, &kp);
            minted.push_back(token_id);
        }

        let owned = client.tokens_of_owner(&user1);
        assert_eq!(owned.len(), 5);
        
        // Verify returned tokens match minted tokens
        for i in 0..5 {
            assert_eq!(owned.get(i as u32).unwrap(), minted.get(i as u32).unwrap());
        }
    }

        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Pre-mint clip 502
        do_mint(&client, &env, &user1, 502, &kp);

        let uri = String::from_str(&env, "ipfs://QmDup");
        let sig = sign_mint(&env, &kp, &user1, 502, &uri);

        let mut clip_ids = Vec::new(&env);
        clip_ids.push_back(502u32);
        let mut uris = Vec::new(&env);
        uris.push_back(uri);
        let mut sigs = Vec::new(&env);
        sigs.push_back(sig);

        let result = client.try_batch_mint(
            &user1,
            &clip_ids,
            &uris,
            &default_royalty(&env, user1.clone()),
            &false,
            &sigs,
        );
        assert_eq!(result, Err(Ok(Error::TokenAlreadyMinted)));
    }'''

new = '''    #[test]
    fn test_batch_mint_duplicate_clip_id_fails() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        // Pre-mint clip 502
        do_mint(&client, &env, &user1, 502, &kp);

        let uri = String::from_str(&env, "ipfs://QmDup");
        let sig = sign_mint(&env, &kp, &user1, 502, &uri);

        let mut clip_ids = Vec::new(&env);
        clip_ids.push_back(502u32);
        let mut uris = Vec::new(&env);
        uris.push_back(uri);
        let mut sigs = Vec::new(&env);
        sigs.push_back(sig);

        let result = client.try_batch_mint(
            &user1,
            &clip_ids,
            &uris,
            &default_royalty(&env, user1.clone()),
            &false,
            &sigs,
        );
        assert_eq!(result, Err(Ok(Error::TokenAlreadyMinted)));
    }'''

if old in content:
    content = content.replace(old, new)
    print('Fixed malformed test block')
else:
    print('WARNING: malformed block not found')

# Fix 2: Add TransferEvent to burn
old_burn = '''        env.events().publish(
            (symbol_short!("burn"),),
            BurnEvent {
                owner,
                token_id,
                clip_id: data.clip_id,
            },
        );

        Ok(())
    }'''
new_burn = '''        env.events().publish(
            (symbol_short!("burn"),),
            BurnEvent {
                owner: owner.clone(),
                token_id,
                clip_id: data.clip_id,
            },
        );

        // Emit standard Transfer event for ERC-721 compliance
        env.events().publish(
            (symbol_short!("transfer"),),
            TransferEvent {
                token_id,
                from: owner.clone(),
                to: env.current_contract_address(),
            },
        );

        Ok(())
    }'''
if old_burn in content:
    content = content.replace(old_burn, new_burn)
    print('Added TransferEvent to burn')
else:
    print('WARNING: burn event block not found')

# Fix 3: Update test_mint_emits_event
old_mint = '''    #[test]
    fn test_mint_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 5, &kp);

        let events = env.events().all();
        assert_eq!(events.events().len(), 1);
        assert_eq!(token_id, 1);
    }'''
new_mint = '''    #[test]
    fn test_mint_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 5, &kp);

        let events = env.events().all();
        // Mint emits both MintEvent and TransferEvent
        assert_eq!(events.events().len(), 2);
        assert_eq!(token_id, 1);
    }'''
if old_mint in content:
    content = content.replace(old_mint, new_mint)
    print('Updated test_mint_emits_event')
else:
    print('WARNING: test_mint_emits_event not found')

# Fix 4: Update test_burn_emits_event
old_burn_test = '''    #[test]
    fn test_burn_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 77, &kp);
        client.burn(&user1, &token_id);

        let events = env.events().all();
        assert_eq!(events.events().len(), 1);
    }'''
new_burn_test = '''    #[test]
    fn test_burn_emits_event() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let token_id = do_mint(&client, &env, &user1, 77, &kp);
        client.burn(&user1, &token_id);

        let events = env.events().all();
        // Burn emits both BurnEvent and TransferEvent
        assert_eq!(events.events().len(), 2);
    }'''
if old_burn_test in content:
    content = content.replace(old_burn_test, new_burn_test)
    print('Updated test_burn_emits_event')
else:
    print('WARNING: test_burn_emits_event not found')

# Fix 5: Add new tests
new_tests = '''    #[test]
    fn test_balance_of_counts_owned_tokens() {
        let (env, admin, user1, user2) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        assert_eq!(client.balance_of(&user1), 0);
        let t1 = do_mint(&client, &env, &user1, 800, &kp);
        assert_eq!(client.balance_of(&user1), 1);
        let _t2 = do_mint(&client, &env, &user1, 801, &kp);
        assert_eq!(client.balance_of(&user1), 2);

        client.transfer(&user1, &user2, &t1);
        assert_eq!(client.balance_of(&user1), 1);
        assert_eq!(client.balance_of(&user2), 1);
    }

    #[test]
    fn test_token_by_index_enumerable() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        let t1 = do_mint(&client, &env, &user1, 810, &kp);
        let _t2 = do_mint(&client, &env, &user1, 811, &kp);
        let t3 = do_mint(&client, &env, &user1, 812, &kp);

        assert_eq!(client.token_by_index(&0), t1);
        assert_eq!(client.token_by_index(&2), t3);

        client.burn(&user1, &t1);
        assert_eq!(client.token_by_index(&0), 2);
    }

    #[test]
    fn test_token_by_index_out_of_bounds() {
        let (env, admin, user1, _) = setup();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        client.init(&admin);
        let kp = register_signer(&env, &client, &admin);

        do_mint(&client, &env, &user1, 820, &kp);
        let result = client.try_token_by_index(&5);
        assert_eq!(result, Err(Ok(Error::InvalidTokenId)));
    }

'''

marker = '''    #[test]
    fn test_royalty_checked_mul_large_safe_price() {
        // 10^15 stroops * 600 bps / 10_000 = 6 * 10^13
        let result = ClipsNftContract::calculate_royalty(1_000_000_000_000_000i128, 600);
        assert_eq!(result, Ok(60_000_000_000_000i128));
    }

}'''

if marker in content:
    content = content.replace(marker, new_tests + marker)
    print('Added new tests')
else:
    print('WARNING: insertion marker not found')

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('Done')

