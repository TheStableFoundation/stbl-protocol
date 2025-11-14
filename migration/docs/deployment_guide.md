# BACH Token Swap - Devnet Deployment Guide

## Prerequisites

1. Install Rust and Solana CLI:
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/v1.18.0/install)"

# Install Anchor
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
avm install latest
avm use latest
```

2. Set up Solana for Devnet:
```bash
solana config set --url https://api.devnet.solana.com
solana-keygen new -o ~/.config/solana/id.json
solana airdrop 2
```

## Project Setup

1. Create a new Anchor project:
```bash
anchor init bach_token_swap
cd bach_token_swap
```

2. Update `Cargo.toml`:
```toml
[dependencies]
anchor-lang = "0.30.1"
anchor-spl = "0.30.1"
```

3. Update `Anchor.toml`:
```toml
[provider]
cluster = "devnet"
wallet = "~/.config/solana/id.json"

[programs.devnet]
bach_token_swap = "YOUR_PROGRAM_ID"  # Will be generated

[[test.validator.account]]
address = "DENNuKzCcrLhEtxZ8tm7nSeef8qvKgGGrdxX6euNkNS7"
filename = "old_bach_token.json"
```

4. Replace the content of `programs/bach_token_swap/src/lib.rs` with the Rust program code.

## Step 1: Deploy the New Token-2022

First, you need to deploy your new BACH token using Token-2022 program:

```bash
# Create the new token mint (Token-2022)
spl-token create-token --program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb --decimals 9

# Save the output address - this is your NEW_BACH_TOKEN
# Example output: Creating token ABC123...
```

Update the token address in your code with the newly created Token-2022 mint address.

## Step 2: Build and Deploy the Program

```bash
# Build the program
anchor build

# Get the program ID
solana address -k target/deploy/bach_token_swap-keypair.json

# Update lib.rs with the program ID from above
# Replace: declare_id!("YOUR_PROGRAM_ID_HERE");

# Build again after updating program ID
anchor build

# Deploy to devnet
anchor deploy --provider.cluster devnet
```

## Step 3: Create Token Vaults

```bash
# Create associated token account for old BACH (vault)
spl-token create-account DENNuKzCcrLhEtxZ8tm7nSeef8qvKgGGrdxX6euNkNS7

# Create associated token account for new BACH (vault) - owned by PDA
# You'll need to do this through the program or manually with the correct owner
```

## Step 4: Initialize the Swap Program

Create a script `scripts/initialize.ts`:

```typescript
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { 
  TOKEN_PROGRAM_ID, 
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";

const OLD_BACH_TOKEN = new PublicKey("DENNuKzCcrLhEtxZ8tm7nSeef8qvKgGGrdxX6euNkNS7");
const NEW_BACH_TOKEN = new PublicKey("YOUR_NEW_TOKEN_2022_ADDRESS");

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  
  const program = anchor.workspace.BachTokenSwap as Program;
  const authority = provider.wallet.publicKey;

  // Derive PDA
  const [swapStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("swap_state")],
    program.programId
  );

  // Get vault addresses
  const oldTokenVault = getAssociatedTokenAddressSync(
    OLD_BACH_TOKEN,
    authority,
    false,
    TOKEN_PROGRAM_ID
  );

  const newTokenVault = getAssociatedTokenAddressSync(
    NEW_BACH_TOKEN,
    swapStatePDA,
    true,
    TOKEN_2022_PROGRAM_ID
  );

  console.log("Creating new token vault...");
  // Create the new token vault account
  const createVaultIx = createAssociatedTokenAccountInstruction(
    authority,
    newTokenVault,
    swapStatePDA,
    NEW_BACH_TOKEN,
    TOKEN_2022_PROGRAM_ID
  );

  const createVaultTx = new anchor.web3.Transaction().add(createVaultIx);
  await provider.sendAndConfirm(createVaultTx);

  console.log("Initializing swap program...");
  const tx = await program.methods
    .initialize()
    .accounts({
      swapState: swapStatePDA,
      authority: authority,
      oldTokenMint: OLD_BACH_TOKEN,
      newTokenMint: NEW_BACH_TOKEN,
      oldTokenVault: oldTokenVault,
      newTokenVault: newTokenVault,
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  console.log("Initialization successful!");
  console.log("Transaction signature:", tx);
  console.log("Swap State PDA:", swapStatePDA.toString());
}

main().then(() => process.exit(0)).catch(console.error);
```

Run the initialization:
```bash
ts-node scripts/initialize.ts
```

## Step 5: Fund the New Token Vault

You need to mint new Token-2022 tokens and send them to the vault:

```bash
# Mint new tokens to your wallet first
spl-token mint YOUR_NEW_TOKEN_2022_ADDRESS 1000000000 --program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb

# Transfer to the vault (PDA's associated token account)
spl-token transfer YOUR_NEW_TOKEN_2022_ADDRESS 1000000000 NEW_TOKEN_VAULT_ADDRESS --fund-recipient --program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb
```

## Step 6: Test the Swap

Create a test script or use the provided test client:

```bash
# Get some old BACH tokens for testing (if you don't have any)
# You'll need to get them from a faucet or mint them if you control the mint

# Run the tests
anchor test --skip-local-validator
```

## Testing Script Example

```typescript
import * as anchor from "@coral-xyz/anchor";

async function testSwap() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  
  const program = anchor.workspace.BachTokenSwap;
  const user = provider.wallet.publicKey;

  // Get PDAs and accounts
  const [swapStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("swap_state")],
    program.programId
  );

  const swapAmount = new anchor.BN(1_000_000_000); // 1 token with 9 decimals

  const tx = await program.methods
    .swapTokens(swapAmount)
    .accounts({
      // ... accounts
    })
    .rpc();

  console.log("Swap successful:", tx);
}

testSwap().catch(console.error);
```

## Troubleshooting

### Common Issues:

1. **Insufficient SOL**: Make sure you have enough SOL for transaction fees
   ```bash
   solana airdrop 2
   ```

2. **Token account doesn't exist**: Create associated token accounts first
   ```bash
   spl-token create-account <MINT_ADDRESS>
   ```

3. **Insufficient tokens in vault**: Mint more tokens to the vault

4. **Wrong program ID**: Make sure `declare_id!()` matches your deployed program

5. **Account validation failed**: Double-check all account addresses and PDAs

## Monitoring

Check your program logs:
```bash
solana logs | grep -i "Program <YOUR_PROGRAM_ID>"
```

Check account balances:
```bash
# Old token vault
spl-token balance DENNuKzCcrLhEtxZ8tm7nSeef8qvKgGGrdxX6euNkNS7 --owner OLD_VAULT_ADDRESS

# New token vault
spl-token balance NEW_TOKEN_ADDRESS --owner NEW_VAULT_ADDRESS --program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb
```

## Important Notes

- **Devnet Old BACH Token**: `DENNuKzCcrLhEtxZ8tm7nSeef8qvKgGGrdxX6euNkNS7`
- **Testnet Old BACH Token**: `A6a2s9LTZcYZQgxrDatLHYfvHhJEfb5ZWuFENhHtxJtR` (for future use)
- The swap ratio is 1:1 by default but can be updated by the authority
- Make sure to fund the new token vault with enough tokens before users start swapping
- The PDA (swap_state) is the authority for the new token vault
- Always test thoroughly on devnet before considering mainnet deployment

## Security Considerations

1. **Test extensively** on devnet with various amounts
2. **Verify** all token mints and addresses
3. **Monitor** vault balances regularly
4. **Consider** adding pause/emergency functions
5. **Audit** the code before mainnet deployment
6. **Implement** proper access controls for admin functions