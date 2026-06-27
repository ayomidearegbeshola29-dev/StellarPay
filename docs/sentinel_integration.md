# Sentinel Cross-Contract Integration

This document outlines the integration boundary and cross-contract authorization model between the **StellarSentinel** governance/treasury contract and the **StellarPay** payroll/vesting system.

## The Model: Native Soroban Authorization (Model B)

StellarPay uses Soroban's native `require_auth()` capability to securely verify authorization from a Sentinel-controlled treasury. 

When a user or automated relayer attempts to fund a payroll stream or vesting schedule using funds from a Sentinel treasury, the transaction must include an `Auth` entry proving that the Sentinel smart contract has approved the action. 

### Flow

1. The Sentinel treasury approves a proposal to fund a specific payroll stream (specifying token, amount, recipient, start/end times).
2. A transaction is submitted invoking `StellarPay::create_stream` with the Sentinel treasury's address as the `sender`.
3. StellarPay calls `sender.require_auth()`.
4. The Soroban host pauses execution and invokes the Sentinel contract's `__check_auth` method, passing the full invocation tree (contract ID = StellarPay, method = `create_stream`, and all arguments).
5. Sentinel's `__check_auth` verifies that the exact arguments match an approved proposal in its state.
6. If Sentinel returns `Ok(())`, StellarPay resumes execution, transfers the tokens from Sentinel to itself, and creates the stream obligation.

## Threat Model & Security Validations

Using Soroban's native `require_auth` guarantees protection against the following attack vectors:

- **Wrong Asset / Amount / Recipient:** The `Auth` tree built by the Soroban host hashes all arguments of the `create_stream` invocation. If an attacker attempts to change the `token` to a worthless asset, or alter the `amount` or `recipient`, the hash verified by Sentinel's `__check_auth` will mismatch the approved proposal. The transaction will safely fail.
- **Wrong Target Contract (Confused Deputy):** The `Auth` tree includes the contract ID of the target (StellarPay). If an attacker tries to use a Sentinel authorization meant for StellarPay on a malicious contract, the signature/authorization will fail to validate.
- **Replay Attacks:** Soroban handles replay protection natively at the protocol level. If the Sentinel contract is a Custom Account, its `__check_auth` mechanism manages nonces, ensuring the exact same authorization cannot be executed twice.
- **Stale Approval:** Sentinel can invalidate or expire proposals in its own state. If `__check_auth` is called for a stale proposal, it returns an error, halting the funding.
- **Failed Token Transfer:** Because token transfers occur synchronously within the `create_stream` logic, if Sentinel does not have enough balance or the token transfer reverts, the entire `create_stream` invocation reverts. No partial obligations or stream metadata will be saved.

## Event Traceability

The cross-contract event trail cleanly maps Sentinel approval to StellarPay obligation:
1. **Approval Event:** Sentinel emits a proposal execution/approval event.
2. **Transfer Event:** The Stellar asset contract emits a token `transfer` event from Sentinel to StellarPay.
3. **Creation Event:** StellarPay emits `s_create` (Stream Creation) or `v_create` (Vesting Creation), referencing the Sentinel address as the sender/grantor.

By scanning the ledger for these three synchronous events within the same transaction footprint, off-chain indexers can definitively reconcile Sentinel governance actions with active StellarPay obligations.
