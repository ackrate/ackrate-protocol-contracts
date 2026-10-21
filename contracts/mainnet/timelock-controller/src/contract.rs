use soroban_sdk::{
    address_payload::AddressPayload, contract, contracterror, contractevent, contractimpl,
    contracttype, panic_with_error, symbol_short, vec, Address, BytesN, Env, IntoVal, Symbol, Val,
};
use stellar_governance::timelock::{
    cancel_operation, execute_operation, get_operation_ledger, get_operation_state, hash_operation,
    schedule_operation, set_min_delay, Operation, OperationState, TimelockStorageKey,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub target: Address,
    pub governance: Address,
    pub initial_admin: Address,
    pub delay: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Upgrade(BytesN<32>),
    Unpause,
    AssetPolicy(Address, bool),
    TransferAdmin(Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    InvalidConfig = 100,
    InvalidHandoff = 101,
    NotOwner = 102,
    DelayOverflow = 103,
    GovernanceFrozen = 104,
}

#[contract]
pub struct TimelockController;

#[contractevent]
pub struct ManagementAccepted {
    pub target: Address,
}

fn config(e: &Env) -> Config {
    let max = e.storage().max_ttl();
    e.storage().instance().extend_ttl(max / 2, max);
    e.storage()
        .instance()
        .get(&symbol_short!("config"))
        .unwrap()
}

fn owned(e: &Env, c: &Config) {
    let admin: Address = e.invoke_contract(&c.target, &Symbol::new(e, "get_admin"), vec![e]);
    if admin != e.current_contract_address() {
        panic_with_error!(e, Error::NotOwner);
    }
}

fn bootstrap(e: &Env, c: &Config) {
    let adopted: bool = e
        .storage()
        .instance()
        .get(&symbol_short!("adopted"))
        .unwrap_or(false);
    let admin: Address = e.invoke_contract(&c.target, &Symbol::new(e, "get_admin"), vec![e]);
    let pending: Option<Address> =
        e.invoke_contract(&c.target, &Symbol::new(e, "get_pending_admin"), vec![e]);
    if adopted || admin != c.initial_admin || pending.is_some() {
        panic_with_error!(e, Error::GovernanceFrozen);
    }
}

fn operation(
    e: &Env,
    c: &Config,
    action: Action,
    predecessor: BytesN<32>,
    salt: BytesN<32>,
) -> Operation {
    let (function, args) = match action {
        Action::Upgrade(hash) => (symbol_short!("upgrade"), vec![e, hash.into_val(e)]),
        Action::Unpause => (symbol_short!("unpause"), vec![e]),
        Action::AssetPolicy(asset, allowed) => (
            Symbol::new(e, "set_asset_allowed"),
            vec![e, asset.into_val(e), allowed.into_val(e)],
        ),
        Action::TransferAdmin(admin) => {
            (Symbol::new(e, "propose_admin"), vec![e, admin.into_val(e)])
        }
    };
    Operation {
        target: c.target.clone(),
        function,
        args,
        predecessor,
        salt,
    }
}

#[contractimpl]
impl TimelockController {
    /// Fixed target and delay. Governance can hand off only before adoption.
    pub fn __constructor(e: Env, target: Address, governance: Address, delay: u32) {
        if delay == 0
            || delay > e.storage().max_ttl() / 2
            || !matches!(target.to_payload(), Some(AddressPayload::ContractIdHash(_)))
            || !matches!(
                governance.to_payload(),
                Some(AddressPayload::AccountIdPublicKeyEd25519(_))
            )
            || target == e.current_contract_address()
        {
            panic_with_error!(&e, Error::InvalidConfig);
        }
        let initial_admin: Address =
            e.invoke_contract(&target, &Symbol::new(&e, "get_admin"), vec![&e]);
        e.storage().instance().set(
            &symbol_short!("config"),
            &Config {
                target,
                governance,
                initial_admin,
                delay,
            },
        );
        set_min_delay(&e, delay);
        config(&e);
    }

    pub fn get_config(e: Env) -> Config {
        config(&e)
    }

    pub fn get_pending_governance(e: Env) -> Option<Address> {
        config(&e);
        e.storage().instance().get(&symbol_short!("pending"))
    }

    pub fn is_adopted(e: Env) -> bool {
        config(&e);
        e.storage()
            .instance()
            .get(&symbol_short!("adopted"))
            .unwrap_or(false)
    }

    /// The recipient must explicitly accept before the managed target nominates us.
    pub fn propose_governance(e: Env, successor: Address) {
        let c = config(&e);
        c.governance.require_auth();
        bootstrap(&e, &c);
        if !matches!(
            successor.to_payload(),
            Some(AddressPayload::AccountIdPublicKeyEd25519(_))
        ) || successor == c.governance
        {
            panic_with_error!(&e, Error::InvalidConfig);
        }
        e.storage()
            .instance()
            .set(&symbol_short!("pending"), &successor);
    }

    pub fn accept_governance(e: Env) {
        let mut c = config(&e);
        bootstrap(&e, &c);
        let successor: Address = e
            .storage()
            .instance()
            .get(&symbol_short!("pending"))
            .unwrap_or_else(|| panic_with_error!(&e, Error::InvalidHandoff));
        successor.require_auth();
        c.governance = successor;
        e.storage().instance().set(&symbol_short!("config"), &c);
        e.storage().instance().remove(&symbol_short!("pending"));
    }

    /// Bootstrap only: accept exactly the handoff from the bound authority.
    /// A direct target invocation cannot impersonate this contract.
    pub fn accept_management(e: Env) {
        let c = config(&e);
        c.governance.require_auth();
        let admin: Address = e.invoke_contract(&c.target, &Symbol::new(&e, "get_admin"), vec![&e]);
        let pending: Option<Address> =
            e.invoke_contract(&c.target, &Symbol::new(&e, "get_pending_admin"), vec![&e]);
        let adopted: bool = e
            .storage()
            .instance()
            .get(&symbol_short!("adopted"))
            .unwrap_or(false);
        let pending_governance: Option<Address> =
            e.storage().instance().get(&symbol_short!("pending"));
        if adopted
            || pending_governance.is_some()
            || admin != c.initial_admin
            || pending != Some(e.current_contract_address())
        {
            panic_with_error!(&e, Error::InvalidHandoff);
        }
        e.invoke_contract::<Val>(&c.target, &Symbol::new(&e, "accept_admin"), vec![&e]);
        owned(&e, &c);
        e.storage().instance().set(&symbol_short!("adopted"), &true);
        ManagementAccepted { target: c.target }.publish(&e);
    }

    /// Emergency stop only. Resumption still passes through the delay.
    pub fn pause_now(e: Env) {
        let c = config(&e);
        c.governance.require_auth();
        owned(&e, &c);
        e.invoke_contract::<Val>(&c.target, &symbol_short!("pause"), vec![&e]);
    }

    pub fn schedule(
        e: Env,
        action: Action,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> BytesN<32> {
        let c = config(&e);
        c.governance.require_auth();
        owned(&e, &c);
        // The upstream primitive saturates this addition. Reject overflow
        // rather than silently shortening the configured waiting period.
        if e.ledger()
            .sequence()
            .checked_add(c.delay)
            .filter(|n| *n > 1)
            .is_none()
        {
            panic_with_error!(&e, Error::DelayOverflow);
        }
        let id = schedule_operation(&e, &operation(&e, &c, action, predecessor, salt), c.delay);
        let max = e.storage().max_ttl();
        e.storage().persistent().extend_ttl(
            &TimelockStorageKey::OperationLedger(id.clone()),
            max / 2,
            max,
        );
        id
    }

    pub fn cancel(e: Env, id: BytesN<32>) {
        let c = config(&e);
        c.governance.require_auth();
        cancel_operation(&e, &id);
    }

    /// Anyone may execute the exact approved payload once it is ready.
    pub fn execute(e: Env, action: Action, predecessor: BytesN<32>, salt: BytesN<32>) -> Val {
        let c = config(&e);
        owned(&e, &c);
        execute_operation(&e, &operation(&e, &c, action, predecessor, salt))
    }

    pub fn operation_id(
        e: Env,
        action: Action,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> BytesN<32> {
        hash_operation(&e, &operation(&e, &config(&e), action, predecessor, salt))
    }

    pub fn state(e: Env, id: BytesN<32>) -> OperationState {
        config(&e);
        get_operation_state(&e, &id)
    }

    pub fn ready_ledger(e: Env, id: BytesN<32>) -> u32 {
        config(&e);
        get_operation_ledger(&e, &id)
    }
}
