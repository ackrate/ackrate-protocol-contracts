#![no_std]
//! Immutable reference extension for Simple MandateRegistry.
//!
//! This contract is deliberately not a universal policy interpreter. A user
//! installs one immutable per-mandate policy containing an executor,
//! per-payment cap, and start time. The extension owns no token allowance,
//! never calls a token, has no administrator or upgrade method, and can invoke
//! only the registry pinned by the installed policy.

#[cfg(test)]
extern crate std;

mod error;
mod registry;
mod storage;
mod types;

pub use error::Error;
pub use registry::{MandateStatus, SimpleMandate, SimpleRegistry, SimpleRegistryClient};
pub use types::{
    ExecutionRequest, InstalledPolicy, PolicyBinding, INTERFACE_VERSION, MAX_POLICY_LIFETIME_SECS,
    MAX_REQUEST_LIFETIME_SECS,
};

use soroban_sdk::{
    contract, contractimpl, xdr::ToXdr, Address, Bytes, BytesN, Env, IntoVal, Symbol,
};

const POLICY_DOMAIN: &[u8] = b"REAPP\0EXTENSION\0POLICY\0V1\0";
const REQUEST_DOMAIN: &[u8] = b"REAPP\0EXTENSION\0REQUEST\0V1\0";

#[contract]
pub struct ReferenceMandateExtension;

#[contractimpl]
impl ReferenceMandateExtension {
    /// Install a policy exactly once after the user has registered a Simple
    /// mandate whose agent is this extension. The live mandate supplies every
    /// base binding so callers cannot substitute merchant, asset, or budget.
    pub fn install_policy(
        env: Env,
        registry: Address,
        mandate_id: BytesN<32>,
        executor: Address,
        max_per_payment: i128,
        not_before: u64,
    ) -> Result<InstalledPolicy, Error> {
        if storage::has_policy(&env, &registry, &mandate_id) {
            return Err(Error::PolicyAlreadyInstalled);
        }

        let mandate = SimpleRegistryClient::new(&env, &registry).get_mandate(&mandate_id);
        if mandate.status != MandateStatus::Active {
            return Err(Error::MandateInactive);
        }
        if mandate.agent != env.current_contract_address() || mandate.vc_hash != mandate_id {
            return Err(Error::MandateMismatch);
        }
        if max_per_payment <= 0 || max_per_payment > mandate.max_amount {
            return Err(Error::InvalidAmount);
        }

        let now = env.ledger().timestamp();
        if mandate.expiry <= now || not_before >= mandate.expiry {
            return Err(Error::InvalidWindow);
        }
        if mandate.expiry - now > MAX_POLICY_LIFETIME_SECS {
            return Err(Error::PolicyTooLong);
        }

        mandate.user.require_auth();
        let binding = PolicyBinding {
            version: INTERFACE_VERSION,
            network_id: env.ledger().network_id(),
            registry,
            extension: env.current_contract_address(),
            mandate_id,
            user: mandate.user,
            merchant: mandate.merchant,
            asset: mandate.asset,
            mandate_max_amount: mandate.max_amount,
            mandate_expiry: mandate.expiry,
            executor,
            max_per_payment,
            not_before,
        };
        let policy_hash = typed_hash(&env, POLICY_DOMAIN, &binding);
        let installed = InstalledPolicy {
            binding,
            policy_hash: policy_hash.clone(),
        };
        storage::set_policy(&env, &installed);
        env.events().publish(
            (
                Symbol::new(&env, "ext_install"),
                installed.binding.registry.clone(),
                installed.binding.mandate_id.clone(),
            ),
            policy_hash,
        );
        Ok(installed)
    }

    pub fn get_policy(
        env: Env,
        registry: Address,
        mandate_id: BytesN<32>,
    ) -> Result<InstalledPolicy, Error> {
        storage::get_policy(&env, &registry, &mandate_id).ok_or(Error::PolicyNotFound)
    }

    pub fn policy_hash(env: Env, binding: PolicyBinding) -> BytesN<32> {
        typed_hash(&env, POLICY_DOMAIN, &binding)
    }

    pub fn request_id(env: Env, request: ExecutionRequest) -> BytesN<32> {
        typed_hash(&env, REQUEST_DOMAIN, &request)
    }

    pub fn is_nonce_consumed(env: Env, policy_hash: BytesN<32>, nonce: BytesN<32>) -> bool {
        storage::is_consumed(&env, &policy_hash, &nonce)
    }

    /// Apply the stricter reference policy, consume its nonce, then enter the
    /// unchanged registry money path. Any downstream rejection rolls back the
    /// nonce and the re-entry lock with the rest of the transaction.
    pub fn execute(env: Env, request: ExecutionRequest) -> Result<(), Error> {
        validate_request_envelope(&env, &request)?;
        let installed = storage::get_policy(&env, &request.registry, &request.mandate_id)
            .ok_or(Error::PolicyNotFound)?;
        validate_against_policy(&env, &request, &installed)?;
        installed.binding.executor.require_auth();

        if storage::is_consumed(&env, &installed.policy_hash, &request.nonce) {
            return Err(Error::Replay);
        }
        if storage::is_executing(&env, &request.registry, &request.mandate_id) {
            return Err(Error::ReentrantExecution);
        }

        let mandate =
            SimpleRegistryClient::new(&env, &request.registry).get_mandate(&request.mandate_id);
        require_live_mandate_agrees(&env, &installed.binding, &mandate)?;

        storage::set_executing(&env, &request.registry, &request.mandate_id);
        storage::set_consumed(
            &env,
            &installed.policy_hash,
            &request.nonce,
            installed.binding.mandate_expiry,
        );

        SimpleRegistryClient::new(&env, &request.registry).execute_payment(
            &request.mandate_id,
            &request.amount,
            &request.expected_seq,
        );

        storage::clear_executing(&env, &request.registry, &request.mandate_id);
        let request_id = typed_hash(&env, REQUEST_DOMAIN, &request);
        env.events().publish(
            (
                Symbol::new(&env, "ext_execute"),
                request.registry,
                request.mandate_id,
            ),
            (request_id, request.amount, request.expected_seq),
        );
        Ok(())
    }
}

fn validate_request_envelope(env: &Env, request: &ExecutionRequest) -> Result<(), Error> {
    if request.version != INTERFACE_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    if request.network_id != env.ledger().network_id() {
        return Err(Error::WrongNetwork);
    }
    if request.extension != env.current_contract_address() {
        return Err(Error::MandateMismatch);
    }
    if request.amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    let now = env.ledger().timestamp();
    if request.valid_after > now
        || request.valid_before <= now
        || request.valid_after >= request.valid_before
    {
        return Err(Error::InvalidWindow);
    }
    if request.valid_before - request.valid_after > MAX_REQUEST_LIFETIME_SECS {
        return Err(Error::RequestTooLong);
    }
    Ok(())
}

fn validate_against_policy(
    env: &Env,
    request: &ExecutionRequest,
    installed: &InstalledPolicy,
) -> Result<(), Error> {
    let binding = &installed.binding;
    if binding.version != INTERFACE_VERSION
        || binding.network_id != request.network_id
        || binding.registry != request.registry
        || binding.extension != request.extension
        || binding.mandate_id != request.mandate_id
    {
        return Err(Error::MandateMismatch);
    }
    if installed.policy_hash != request.policy_hash
        || typed_hash(env, POLICY_DOMAIN, binding) != installed.policy_hash
    {
        return Err(Error::PolicyHashMismatch);
    }
    let now = env.ledger().timestamp();
    if now < binding.not_before || request.valid_before > binding.mandate_expiry {
        return Err(Error::InvalidWindow);
    }
    if request.amount > binding.max_per_payment {
        return Err(Error::PaymentCapExceeded);
    }
    Ok(())
}

fn require_live_mandate_agrees(
    env: &Env,
    binding: &PolicyBinding,
    mandate: &SimpleMandate,
) -> Result<(), Error> {
    if mandate.status != MandateStatus::Active {
        return Err(Error::MandateInactive);
    }
    if mandate.user != binding.user
        || mandate.agent != env.current_contract_address()
        || mandate.merchant != binding.merchant
        || mandate.asset != binding.asset
        || mandate.max_amount != binding.mandate_max_amount
        || mandate.expiry != binding.mandate_expiry
        || mandate.vc_hash != binding.mandate_id
    {
        return Err(Error::MandateMismatch);
    }
    Ok(())
}

fn typed_hash<T>(env: &Env, domain: &[u8], value: &T) -> BytesN<32>
where
    T: Clone + IntoVal<Env, soroban_sdk::Val>,
{
    let mut bytes = Bytes::from_slice(env, domain);
    bytes.append(&value.clone().to_xdr(env));
    env.crypto().sha256(&bytes).into()
}

#[cfg(test)]
mod test;
