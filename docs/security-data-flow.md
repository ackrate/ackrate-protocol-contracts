# Mainnet security data flow

Status: release gate
Last reviewed: 2026-08-27

## System and trust boundaries

```mermaid
flowchart LR
    U["User + Freighter\ntrusted signer"]
    UI["Web wallet\nuntrusted presentation"]
    A["Consumer agent + SDK\nuntrusted caller"]
    X["x402 adapter\nuntrusted wire format"]
    M["Fulfillment agent / merchant\nuntrusted service"]
    R["MandateRegistry\non-chain enforcement boundary"]
    T["Circle USDC SAC\nallowed asset"]
    G["Live 2-of-3 authority"]
    L["TimelockController"]
    P["Emergency pauser"]

    U -->|"sign session, mandate, allowance"| UI
    UI -->|"register mandate; approve registry"| R
    A -->|"request protected resource"| X
    X -->|"HTTP 402 challenge / proof"| M
    A -->|"execute_payment(id, amount, seq)"| R
    R -->|"atomic transfer_from(user, merchant)"| T
    M -->|"HTTP 200 only after verified settlement"| A
    G -->|"schedule / cancel"| L
    L -->|"delayed policy or upgrade call"| R
    P -->|"pause only"| R
```

The boundary is the registry transaction, not the SDK call and not the HTTP
exchange. Replacing x402 v0.1 with a later request/response shape does not
change the mandate schema or the payment invariant.

The live canary already uses the technical 2-of-3 authority shown in the
deployment manifest. The remaining step is the independent physical Freighter
custody handoff; it is tracked separately from the on-chain signer math.

## Mandate lifecycle and payment

```mermaid
sequenceDiagram
    autonumber
    participant U as User / Freighter
    participant UI as Web wallet
    participant R as MandateRegistry
    participant A as Consumer agent
    participant M as Fulfillment merchant
    participant S as Circle USDC SAC

    U->>UI: Choose merchant, agent, budget, expiry
    UI->>U: Request transaction signature
    U->>R: register_mandate(signed fields)
    R->>R: Store Active, spent=0, seq=0
    UI->>U: Request USDC allowance signature
    U->>S: approve(spender=MandateRegistry, max_amount)
    A->>M: Request protected resource
    M-->>A: 402 + bound challenge
    A->>R: execute_payment(id, amount, expected_seq)
    R->>R: Authenticate agent and re-read durable mandate
    R->>R: Check status, expiry, merchant, budget, sequence
    R->>R: Increment spent and sequence
    R->>S: transfer_from(user -> merchant)
    S-->>R: Transfer succeeds
    R-->>A: Payment event + transaction receipt
    A->>M: Retry with settlement proof
    M-->>A: 200 + protected result
```

All state consumption and value movement are atomic. If authorization,
validation, sequence comparison, allowance, or token transfer fails, Soroban
reverts the complete invocation.

## State entities

| Entity | Authoritative fields | Writer | Reader |
|---|---|---|---|
| Mandate | user, agent, merchant, asset, max amount, spent, expiry, sequence, status, credential hash | Registry | Registry; public clients for inspection |
| Asset policy | asset address -> allowed | Timelock-authorized registry call | Registry registration path |
| Pause state | boolean | Pauser / unpauser roles | Registry payment path |
| Timelock operation | target, function, args, predecessor, salt, ready ledger, state | Live 2-of-3 proposer; controller | Controller execution path; public observers |
| USDC allowance | owner, spender=registry, amount, expiry ledger | User signature | Circle USDC SAC during transfer |
| Settlement receipt | transaction hash, registry event, token transfer, mandate ID, sequence | Stellar ledger | Merchant verifier and reviewer |
| x402 challenge | method, resource, merchant, asset, amount, nonce/expiry | Merchant adapter | Consumer and fulfillment adapter only; never contract authority |

## Failure flows

```mermaid
flowchart TD
    Q["Payment requested"] --> C{"On-chain checks pass?"}
    C -->|"No: caller / expiry / scope / budget / sequence / pause"| X["Contract rejects; no state or value change"]
    C -->|Yes| D["Consume mandate state"]
    D --> T{"USDC transfer succeeds?"}
    T -->|No| B["Transaction rolls back state and value"]
    T -->|Yes| E["Ledger receipt and event"]
    E --> F{"Merchant reachable?"}
    F -->|No| R["Retry fulfillment using same bound receipt"]
    F -->|Yes| H["HTTP 200 + protected result"]
```

Required live drills document four user-visible outcomes: a normal payment, an
agent operating within its budget, merchant downtime after settlement with
idempotent recovery, and an expired or exhausted mandate rejected without a
transfer.

## Governance flow

```mermaid
sequenceDiagram
    participant A as Signer A
    participant B as Signer B
    participant G as Live 2-of-3 authority
    participant L as TimelockController
    participant R as MandateRegistry

    A->>G: Sign exact reviewed operation
    B->>G: Independently verify and co-sign
    G->>L: schedule(target, function, args, predecessor, salt)
    L->>L: Enforce minimum 17,280-ledger delay
    L->>R: Execute exact ready operation
    R->>R: Enforce timelock-held role
```

The emergency pauser is intentionally outside this sequence and can only stop
the money path. It cannot restore service, alter asset policy, or replace WASM.
