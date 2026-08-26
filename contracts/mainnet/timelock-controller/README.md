# ACKRATE Timelock Controller

This crate vendors the OpenZeppelin Stellar Contracts `timelock-controller`
example from version `0.7.2` at commit
`a9c42169000638da937577f592ebf61a7a3c94ca`.

The mainnet deployment profile is intentionally narrow:

- the controller is self-administered (`admin = None`);
- the selected 2-of-3 authority is the proposer and canceller;
- the executor list is empty, so anyone may execute an operation after its
  delay;
- every operation ID binds the target contract, function, complete argument
  list, predecessor, and salt.

There is no bootstrap administrator in the deployment profile. Changes to
roles or delay use the same scheduled operation lifecycle as registry changes.

The upstream source is MIT licensed:
<https://github.com/OpenZeppelin/stellar-contracts/tree/v0.7.2/examples/timelock-controller>
