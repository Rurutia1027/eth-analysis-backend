## Slot and Epoch Basics

### Slot

- A slot is the smallest time unit in the Beacon Chain, occurring every 12 seconds.
- Each slot can contain a **Beacon Block**, but not every slot must have a block.
- Blocks are identified by their **slot number**, which increases sequentially.

### Epoch

- An **epoch** consists of **32 slots** (~6.4 minutes).
- Validators are shuffled and assigned to different duties at the start of each epoch.
- Rewards and penalties are processed at the epoch level.

--- 

## Deposits and Their Role

### Deposit Overview

- Deposits originate from the **Ethereum Execution Layer(EL)** (Ethereum 1.0).
- They represent ETH being staked and moved from **Ethereum Mainnet(EL)** to the **Beacon Chain(CL)**.
- The minimum **deposit amount is 32 ETH**, required for becoming a validator.

### Deposit in Blocks

- Deposits are included in **Beacon Blocks**.
- A block can contain multiple deposit records, stored in the deposits field.
- Each deposit increases the stake of a validator and is referenced for rewards.

### Relationship with Block Parent

- A block's parent root is a hash reference to its parent block.
- The `state_root` within a block reflects the entire chain state after processing the deposits and other operations.
- A block's deposits contribute to the validator balances recorded in the `state_root`.

--- 

## Withdrawals and Issuance Blocks

### Withdrawals

- Withdrawals refer to the process where validators exit and withdraw their staked ETH.
- This is only possible for after Shanghai & Capella upgrades(EIP-4895).
- Withdrawals are recorded in the withdrawals field of a Beacon Block.

### Issuance Blocks

- Blocks that include newly issued ETH via staking rewards.
- Unlike PoW, where issuance comes from mining, in Proof-of-Stake(PoS), issuance happens through rewards to validators.
- Issuance modifies the state_root, as validator balances increase.

### Relationship Between Withdrawals and Deposits

- Deposits increase a validator's balance, recorded in `state_root`.
- Withdrawals decrease a validator's balance, also modifying `state_root`.
- Both deposits and withdrawals are integral to updating **validator balances** in the Beacon Chain.

--- 

## Block Parent and state_root

### Block Parent

- Every block has a parent block, referenced by its `parent_root`(hash of the previous block).
- The Beacon Chain enforces a **strict chain rule**, ensuring each block links to a valid parent.

### State Root (state_root)

- **Merkle root** representing the full state of the Beacon Chain at that block.
- Includes:
    - Validator balances.
    - Validators status (active, slashed, exited)
    - Latest attestations
    - Historical data (deposits, withdrawals)

### State Transition with Deposits and Withdrawals

- Block is proposed at a slot.
- Parent state_root is retrieved (previous block's finalized state).
- Deposits are processed, increasing validator balances.
- Withdrawals are processed, decreasing validator balances.
- State transactions update the `state_root`, committing the new chain state.

--- 

## Summary

- **Slot**:
    - A time unit(12s) where a block can be proposed.
    - Blocks exist within slots.
- **Beacon Block**:
    - A block in the Beacon Chain containing state transitions.
    - Contains deposits, withdrawals, and `state_root`.
- **Deposit**:
    - ETH moved from Ethereum Mainnet to Beacon Chain.
    - Increases validator balances and updates `state_root`.
- **Withdrawal**:
    - ETH unstaked and sent back to Ethereum Mainnet.
    - Decreases validator balances and updates `state_root`.
- **Parent Root**:
    - Reference to the previous block's hash.
    - Ensures blockchain continuity.
- **State Root**:
    - Merkle root of chain state after processing a block.
    - Stores validator balances, deposits, and withdrawals.


- The **Beacon Chain** follows a **stateful** model where every block transition modifies the global state.
- **Deposits** and **withdrawals** directly impact the **state_root** by updating validator balances.
- Each block's **parent_root** ensures sequential block validation.
- **Issuance** and **withdrawals** define validator economics, influencing network security.

---