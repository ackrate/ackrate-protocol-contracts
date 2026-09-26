extern crate std;
use super::*;
use mandate_registry::{MandateRegistry, MandateRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    IntoVal,
};

struct Fixture {
    env: Env,
    user: Address,
    agent: Address,
    merchant: Address,
    registry: Address,
    asset: Address,
    setup: Address,
}
impl Fixture {
    fn new() -> Self {
        let env = Env::default();
        env.ledger().with_mut(|l| {
            l.timestamp = 1000;
            l.sequence_number = 100;
        });
        let admin = Address::generate(&env);
        let asset = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let registry = env.register(MandateRegistry, (admin, asset.clone()));
        let setup = env.register(WalletSetup, (registry.clone(), asset.clone()));
        Self {
            user: Address::generate(&env),
            agent: Address::generate(&env),
            merchant: Address::generate(&env),
            env,
            registry,
            asset,
            setup,
        }
    }
    fn run(&self, amount: i128, expiration: u32) -> BytesN<32> {
        WalletSetupClient::new(&self.env, &self.setup).register_and_approve(
            &self.user,
            &self.agent,
            &self.merchant,
            &amount,
            &2000,
            &BytesN::from_array(&self.env, &[1; 32]),
            &expiration,
        )
    }
}

#[test]
fn exact_nested_authorization_registers_and_approves_without_moving_funds() {
    let f = Fixture::new();
    let e = &f.env;
    let vc = BytesN::from_array(e, &[1; 32]);
    let amount = 300000i128;
    let expiry = 2000u64;
    let expiration = 300u32;
    // Exact invocation tree. No mock_all_auths: omission of either child must fail.
    e.mock_auths(&[MockAuth {
        address: &f.user,
        invoke: &MockAuthInvoke {
            contract: &f.setup,
            fn_name: "register_and_approve",
            args: (
                &f.user,
                &f.agent,
                &f.merchant,
                amount,
                expiry,
                &vc,
                expiration,
            )
                .into_val(e),
            sub_invokes: &[
                MockAuthInvoke {
                    contract: &f.registry,
                    fn_name: "register_mandate",
                    args: (
                        &f.user,
                        &f.agent,
                        &f.merchant,
                        &f.asset,
                        amount,
                        expiry,
                        &vc,
                    )
                        .into_val(e),
                    sub_invokes: &[],
                },
                MockAuthInvoke {
                    contract: &f.asset,
                    fn_name: "approve",
                    args: (&f.user, &f.registry, amount, expiration).into_val(e),
                    sub_invokes: &[],
                },
            ],
        },
    }]);
    let id = f.run(amount, expiration);
    let mandate = MandateRegistryClient::new(e, &f.registry).get_mandate(&id);
    assert_eq!(mandate.max_amount, amount);
    assert_eq!(mandate.spent, 0);
    let token = token::TokenClient::new(e, &f.asset);
    assert_eq!(token.allowance(&f.user, &f.registry), amount);
    assert_eq!(token.allowance(&f.user, &f.setup), 0);
    assert_eq!(token.allowance(&f.user, &f.agent), 0);
    assert_eq!(token.balance(&f.user), 0);
    assert_eq!(token.balance(&f.merchant), 0);
}

#[test]
fn missing_authorization_is_rejected() {
    let f = Fixture::new();
    assert!(WalletSetupClient::new(&f.env, &f.setup)
        .try_register_and_approve(
            &f.user,
            &f.agent,
            &f.merchant,
            &300000,
            &2000,
            &BytesN::from_array(&f.env, &[1; 32]),
            &300
        )
        .is_err());
    assert_eq!(
        token::TokenClient::new(&f.env, &f.asset).allowance(&f.user, &f.registry),
        0
    );
}

#[test]
fn expired_unbounded_and_nonpositive_approvals_are_rejected() {
    let f = Fixture::new();
    f.env.mock_all_auths();
    for (amount, expiration) in [(0i128, 300u32), (-1, 300), (300000, 100), (300000, 17381)] {
        assert!(WalletSetupClient::new(&f.env, &f.setup)
            .try_register_and_approve(
                &f.user,
                &f.agent,
                &f.merchant,
                &amount,
                &2000,
                &BytesN::from_array(&f.env, &[1; 32]),
                &expiration
            )
            .is_err());
    }
    assert_eq!(
        token::TokenClient::new(&f.env, &f.asset).allowance(&f.user, &f.registry),
        0
    );
}

#[test]
fn missing_token_authorization_rolls_back_registration() {
    let f = Fixture::new();
    let e = &f.env;
    let vc = BytesN::from_array(e, &[1; 32]);
    // The user approves registration but not the token child call.
    e.mock_auths(&[MockAuth {
        address: &f.user,
        invoke: &MockAuthInvoke {
            contract: &f.setup,
            fn_name: "register_and_approve",
            args: (
                &f.user,
                &f.agent,
                &f.merchant,
                300000i128,
                2000u64,
                &vc,
                300u32,
            )
                .into_val(e),
            sub_invokes: &[MockAuthInvoke {
                contract: &f.registry,
                fn_name: "register_mandate",
                args: (
                    &f.user,
                    &f.agent,
                    &f.merchant,
                    &f.asset,
                    300000i128,
                    2000u64,
                    &vc,
                )
                    .into_val(e),
                sub_invokes: &[],
            }],
        },
    }]);
    assert!(WalletSetupClient::new(e, &f.setup)
        .try_register_and_approve(&f.user, &f.agent, &f.merchant, &300000, &2000, &vc, &300)
        .is_err());
    // The same registration can succeed after exact failure: it was rolled back.
    e.mock_all_auths();
    let id = f.run(300000, 300);
    assert_eq!(
        MandateRegistryClient::new(e, &f.registry)
            .get_mandate(&id)
            .spent,
        0
    );
}
