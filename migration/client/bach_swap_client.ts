import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { 
  PublicKey, 
  Keypair, 
  SystemProgram,
  LAMPORTS_PER_SOL
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  getAccount,
  getMint,
} from "@solana/spl-token";
import { BachTokenSwap } from "./target/types/bach_token_swap";

// Devnet addresses
const OLD_BACH_TOKEN = new PublicKey("DENNuKzCcrLhEtxZ8tm7nSeef8qvKgGGrdxX6euNkNS7");
const NEW_BACH_TOKEN = new PublicKey("YOUR_NEW_TOKEN_2022_ADDRESS"); // Deploy this first

describe("BACH Token Swap - Devnet Test", () => {
  // Configure the client to use devnet
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.BachTokenSwap as Program<BachTokenSwap>;
  const authority = provider.wallet as anchor.Wallet;

  // PDAs
  let swapStatePDA: PublicKey;
  let swapStateBump: number;

  // Token accounts
  let oldTokenVault: PublicKey;
  let newTokenVault: PublicKey;
  let userOldTokenAccount: PublicKey;
  let userNewTokenAccount: PublicKey;

  before(async () => {
    console.log("Program ID:", program.programId.toString());
    console.log("Authority:", authority.publicKey.toString());

    // Derive PDA for swap state
    [swapStatePDA, swapStateBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("swap_state")],
      program.programId
    );

    console.log("Swap State PDA:", swapStatePDA.toString());

    // Get associated token accounts
    oldTokenVault = getAssociatedTokenAddressSync(
      OLD_BACH_TOKEN,
      authority.publicKey,
      false,
      TOKEN_PROGRAM_ID
    );

    newTokenVault = getAssociatedTokenAddressSync(
      NEW_BACH_TOKEN,
      swapStatePDA,
      true,
      TOKEN_2022_PROGRAM_ID
    );

    userOldTokenAccount = getAssociatedTokenAddressSync(
      OLD_BACH_TOKEN,
      authority.publicKey,
      false,
      TOKEN_PROGRAM_ID
    );

    userNewTokenAccount = getAssociatedTokenAddressSync(
      NEW_BACH_TOKEN,
      authority.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID
    );

    console.log("Old Token Vault:", oldTokenVault.toString());
    console.log("New Token Vault:", newTokenVault.toString());
  });

  it("Initialize swap program", async () => {
    try {
      const tx = await program.methods
        .initialize()
        .accounts({
          swapState: swapStatePDA,
          authority: authority.publicKey,
          oldTokenMint: OLD_BACH_TOKEN,
          newTokenMint: NEW_BACH_TOKEN,
          oldTokenVault: oldTokenVault,
          newTokenVault: newTokenVault,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      console.log("Initialize transaction signature:", tx);
      
      // Fetch and display swap state
      const swapState = await program.account.swapState.fetch(swapStatePDA);
      console.log("Swap State:", {
        authority: swapState.authority.toString(),
        oldTokenMint: swapState.oldTokenMint.toString(),
        newTokenMint: swapState.newTokenMint.toString(),
        swapRatio: `${swapState.swapRatioNumerator}:${swapState.swapRatioDenominator}`,
        totalSwapped: swapState.totalSwapped.toString(),
      });

    } catch (error) {
      console.error("Error initializing:", error);
      throw error;
    }
  });

  it("Swap tokens (1:1 ratio)", async () => {
    const swapAmount = new anchor.BN(1_000_000); // 1 token (assuming 6 decimals)

    try {
      // Check balances before
      const oldTokenAccountBefore = await getAccount(
        provider.connection,
        userOldTokenAccount,
        "confirmed",
        TOKEN_PROGRAM_ID
      );
      
      console.log("Old BACH balance before:", oldTokenAccountBefore.amount.toString());

      // Perform swap
      const tx = await program.methods
        .swapTokens(swapAmount)
        .accounts({
          swapState: swapStatePDA,
          user: authority.publicKey,
          userOldTokenAccount: userOldTokenAccount,
          userNewTokenAccount: userNewTokenAccount,
          oldTokenVault: oldTokenVault,
          newTokenVault: newTokenVault,
          oldTokenMint: OLD_BACH_TOKEN,
          newTokenMint: NEW_BACH_TOKEN,
          tokenProgram: TOKEN_PROGRAM_ID,
          token2022Program: TOKEN_2022_PROGRAM_ID,
        })
        .rpc();

      console.log("Swap transaction signature:", tx);

      // Check balances after
      const oldTokenAccountAfter = await getAccount(
        provider.connection,
        userOldTokenAccount,
        "confirmed",
        TOKEN_PROGRAM_ID
      );

      const newTokenAccountAfter = await getAccount(
        provider.connection,
        userNewTokenAccount,
        "confirmed",
        TOKEN_2022_PROGRAM_ID
      );

      console.log("Old BACH balance after:", oldTokenAccountAfter.amount.toString());
      console.log("New BACH balance after:", newTokenAccountAfter.amount.toString());

      // Fetch updated swap state
      const swapState = await program.account.swapState.fetch(swapStatePDA);
      console.log("Total swapped:", swapState.totalSwapped.toString());

    } catch (error) {
      console.error("Error swapping tokens:", error);
      throw error;
    }
  });

  it("Update swap ratio (admin only)", async () => {
    const newNumerator = new anchor.BN(2);
    const newDenominator = new anchor.BN(1);

    try {
      const tx = await program.methods
        .updateSwapRatio(newNumerator, newDenominator)
        .accounts({
          swapState: swapStatePDA,
          authority: authority.publicKey,
        })
        .rpc();

      console.log("Update ratio transaction signature:", tx);

      const swapState = await program.account.swapState.fetch(swapStatePDA);
      console.log("New swap ratio:", `${swapState.swapRatioNumerator}:${swapState.swapRatioDenominator}`);

    } catch (error) {
      console.error("Error updating ratio:", error);
      throw error;
    }
  });

  it("Check vault balances", async () => {
    try {
      const oldVaultAccount = await getAccount(
        provider.connection,
        oldTokenVault,
        "confirmed",
        TOKEN_PROGRAM_ID
      );

      const newVaultAccount = await getAccount(
        provider.connection,
        newTokenVault,
        "confirmed",
        TOKEN_2022_PROGRAM_ID
      );

      console.log("Old token vault balance:", oldVaultAccount.amount.toString());
      console.log("New token vault balance:", newVaultAccount.amount.toString());

    } catch (error) {
      console.error("Error checking vault balances:", error);
      throw error;
    }
  });
});

// Standalone functions for manual testing

export async function setupProgram() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  return anchor.workspace.BachTokenSwap as Program<BachTokenSwap>;
}

export async function createUserTokenAccounts(
  provider: anchor.AnchorProvider,
  user: PublicKey
) {
  const instructions = [];

  const oldTokenAccount = getAssociatedTokenAddressSync(
    OLD_BACH_TOKEN,
    user,
    false,
    TOKEN_PROGRAM_ID
  );

  const newTokenAccount = getAssociatedTokenAddressSync(
    NEW_BACH_TOKEN,
    user,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  // Check if accounts exist, if not create them
  try {
    await getAccount(provider.connection, oldTokenAccount, "confirmed", TOKEN_PROGRAM_ID);
  } catch {
    instructions.push(
      createAssociatedTokenAccountInstruction(
        user,
        oldTokenAccount,
        user,
        OLD_BACH_TOKEN,
        TOKEN_PROGRAM_ID
      )
    );
  }

  try {
    await getAccount(provider.connection, newTokenAccount, "confirmed", TOKEN_2022_PROGRAM_ID);
  } catch {
    instructions.push(
      createAssociatedTokenAccountInstruction(
        user,
        newTokenAccount,
        user,
        NEW_BACH_TOKEN,
        TOKEN_2022_PROGRAM_ID
      )
    );
  }

  return { instructions, oldTokenAccount, newTokenAccount };
}

export async function airdropSol(
  connection: anchor.web3.Connection,
  publicKey: PublicKey,
  amount: number = 2
) {
  const signature = await connection.requestAirdrop(
    publicKey,
    amount * LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(signature);
  console.log(`Airdropped ${amount} SOL to ${publicKey.toString()}`);
}