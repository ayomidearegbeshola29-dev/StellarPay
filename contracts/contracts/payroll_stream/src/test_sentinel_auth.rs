#![cfg(test)]

use crate::{PayrollStreamContract, PayrollStreamContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env, IntoVal, Symbol,
};
use soroban_sdk::testutils::MockAuth;
use soroban_sdk::testutils::MockAuthInvoke;

fn create_token_contract<'a>(e: &Env, admin: &Address) -> token::StellarAssetClient<'a> {
    let contract_addr = e.register_stellar_asset_contract(admin.clone());
    token::StellarAssetClient::new(e, &contract_addr)
}

fn create_token_client<'a>(e: &Env, contract_addr: &Address) -> token::Client<'a> {
    token::Client::new(e, contract_addr)
}

#[test]
fn test_sentinel_authorized_funding() {
    let env = Env::default();
    
    // Disable mock_all_auths so we can test explicit authorization
    // env.mock_all_auths(); 

    let contract_id = env.register(PayrollStreamContract, ());
    let client = PayrollStreamContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    
    // "Sentinel" treasury acts as the sender/grantor
    let sentinel_treasury = Address::generate(&env);
    let recipient = Address::generate(&env);
    
    let token_admin = Address::generate(&env);
    let token_admin_client = create_token_contract(&env, &token_admin);
    let token = token_admin_client.address.clone();
    let token_client = create_token_client(&env, &token);
    
    // Sentinel holds treasury funds
    env.mock_all_auths(); // temporarily enable to mint and initialize
    client.initialize(&admin);
    token_admin_client.mint(&sentinel_treasury, &100000);
    
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    // We simulate Sentinel authorizing the stream creation via native Soroban require_auth
    let amount: i128 = 10000;
    let start_time: u64 = 1000;
    let end_time: u64 = 2000;

    // Use MockAuth to simulate the Sentinel smart contract authorizing the call.
    // In production, the Soroban host ensures that if any argument differs, the transaction reverts.
    client
        .mock_auths(&[MockAuth {
            address: &sentinel_treasury,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "create_stream",
                args: (
                    sentinel_treasury.clone(),
                    recipient.clone(),
                    token.clone(),
                    amount,
                    start_time,
                    end_time,
                )
                    .into_val(&env),
                sub_invokes: &[
                    // token_client.transfer will also require auth from sentinel_treasury
                    MockAuthInvoke {
                        contract: &token,
                        fn_name: "transfer",
                        args: (
                            sentinel_treasury.clone(),
                            contract_id.clone(),
                            amount,
                        ).into_val(&env),
                        sub_invokes: &[],
                    }
                ],
            },
        }])
        .create_stream(
            &sentinel_treasury,
            &recipient,
            &token,
            &amount,
            &start_time,
            &end_time,
        );

    // Verify stream was created successfully and Sentinel treasury was debited
    assert_eq!(client.get_stream_count(), 1);
    let stream = client.get_stream(&0);
    assert_eq!(stream.total_amount, amount);
    assert_eq!(token_client.balance(&sentinel_treasury), 90000);
    assert_eq!(token_client.balance(&contract_id), 10000);
}
