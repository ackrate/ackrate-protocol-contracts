#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, Address, BytesN, Env, String};

#[contract]
pub struct Hello;

fn admin(e: &Env) -> Address {
    e.storage().instance().get(&symbol_short!("admin")).unwrap()
}

#[contractimpl]
impl Hello {
    pub fn __constructor(e: Env, authority: Address) {
        e.storage()
            .instance()
            .set(&symbol_short!("admin"), &authority);
        e.storage().instance().set(&symbol_short!("paused"), &false);
        e.storage().instance().set(&symbol_short!("value"), &42u32);
    }
    pub fn hello(e: Env) -> String {
        String::from_str(&e, "Hello World")
    }
    pub fn value(e: Env) -> u32 {
        e.storage().instance().get(&symbol_short!("value")).unwrap()
    }
    pub fn get_admin(e: Env) -> Address {
        admin(&e)
    }
    pub fn get_pending_admin(e: Env) -> Option<Address> {
        e.storage().instance().get(&symbol_short!("pending"))
    }
    pub fn propose_admin(e: Env, new_admin: Address) {
        admin(&e).require_auth();
        e.storage()
            .instance()
            .set(&symbol_short!("pending"), &new_admin);
    }
    pub fn accept_admin(e: Env) {
        let pending: Address = e
            .storage()
            .instance()
            .get(&symbol_short!("pending"))
            .unwrap();
        pending.require_auth();
        e.storage()
            .instance()
            .set(&symbol_short!("admin"), &pending);
        e.storage().instance().remove(&symbol_short!("pending"));
    }
    pub fn is_paused(e: Env) -> bool {
        e.storage()
            .instance()
            .get(&symbol_short!("paused"))
            .unwrap()
    }
    pub fn pause(e: Env) {
        admin(&e).require_auth();
        e.storage().instance().set(&symbol_short!("paused"), &true);
    }
    pub fn unpause(e: Env) {
        admin(&e).require_auth();
        e.storage().instance().set(&symbol_short!("paused"), &false);
    }
    pub fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        admin(&e).require_auth();
        assert!(Self::is_paused(e.clone()), "pause required");
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}
