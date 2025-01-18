import * as anchor from "@coral-xyz/anchor";
import BN from "bn.js";
import assert, { throws } from "assert";
import * as web3 from "@solana/web3.js";
import {ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID} from '@solana/spl-token';
import type { StakingPool } from "../target/types/staking_pool";
import fs from 'fs';
require('dotenv').config();

describe("Test", () => {
  // Configure the client to use the local cluster
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const wallet = provider.wallet;
  const program = anchor.workspace.StakingPool as anchor.Program<StakingPool>;
  const secretKeyArray = JSON.parse(fs.readFileSync(process.env.ANCHOR_WALLET).toString());
  const signer = web3.Keypair.fromSecretKey(Uint8Array.from(secretKeyArray));
  const [pool, bump] = web3.PublicKey.findProgramAddressSync([Buffer.from("pool")], program.programId);
  const tokenAddress = "5C9o1ecjwDWaVZkv2hnwusFferXR4G1udVdUfoNMXxwc";
  const userTokenAccount = getAssociatedTokenAddressSync(new web3.PublicKey(tokenAddress), wallet.publicKey, false, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);
  const [poolTokenAccount, pool_token_account_bump] = web3.PublicKey.findProgramAddressSync([pool.toBuffer()], program.programId);
  const [userAccount, user_account_bump] =  web3.PublicKey.findProgramAddressSync([Buffer.from("user"), signer.publicKey.toBuffer()], program.programId);
  
  it("initialize", async () => {
    // Send transaction
    const data = new BN(1_000_000_000);
    let txHash = await program.methods
      .initialize(data)
      .accounts({
        poolToken: new web3.PublicKey(tokenAddress),
        initializer: signer.publicKey,
      })
      .signers([signer])
      .rpc()
    
    // Confirm transaction
    await program.provider.connection.confirmTransaction(txHash);

    // Fetch the created account
    const poolAccount = await program.account.pool.fetch(
      pool
    );

    console.log("On-chain data is:", poolAccount.totalStaked.toString());

    // Check whether the data on-chain is equal to local 'data'
    assert(poolAccount.totalStaked.eq(new BN(0)));
    assert(poolAccount.totalRewards.eq(new BN(1_000_000_000)));
    assert((await program.provider.connection.getTokenAccountBalance(poolTokenAccount)).value.uiAmount == 1000);
  });

  it("deposit", async () => {
    // Send transaction
    const data = new BN(1_000_000_000);
    let txHash = await program.methods
      .deposit(data)
      .accounts({
        poolToken: new web3.PublicKey(tokenAddress),
        user: signer.publicKey
      })
      .signers([signer]).rpc();
    
    // Confirm transaction
    await program.provider.connection.confirmTransaction(txHash);

    // Fetch the created account
    const poolAccount = await program.account.pool.fetch(
      pool
    );

    console.log("On-chain pool account data is:", poolAccount.totalStaked.toString());

    const userAccountData = await program.account.userAccount.fetch(
      userAccount
    );

    console.log("On-chain user account data is:", userAccountData.amountStaked.toString());

    // Check whether the data on-chain is equal to local 'data'
    assert(poolAccount.totalStaked.eq(new BN(1_000_000_000)));
    assert(userAccountData.amountStaked.eq(new BN(1_000_000_000)));
  });

  it("staked_at should be the last time when deposit again", async () => {
    // Send transaction
    const data = new BN(1_000_000_000);
    let txHash = await program.methods
      .deposit(data)
      .accounts({
        poolToken: new web3.PublicKey(tokenAddress),
        user: signer.publicKey
      })
      .signers([signer]).rpc();
    
    // Confirm transaction
    await program.provider.connection.confirmTransaction(txHash);

    // Fetch the created account
    const poolAccount = await program.account.pool.fetch(
      pool
    );

    console.log("On-chain pool account data is:", poolAccount.totalStaked.toString());

    const userAccountData = await program.account.userAccount.fetch(
      userAccount
    );

    console.log("On-chain user account data is:", userAccountData.amountStaked.toString());

    // Check whether the data on-chain is equal to local 'data'
    assert(poolAccount.totalStaked.eq(new BN(2_000_000_000)));
    assert(userAccountData.amountStaked.eq(new BN(2_000_000_000)));
    assert(Math.abs(parseInt(userAccountData.staked_at.toString()) * 1000 - Date.now()) < 1000 * 60);
  });

  it("show error at withdraw before 1 month", async () => {
    // expect withdraw function throws an error because 1 month did not pass.
    throws(async () => {
      let txHash = await program.methods
        .withdraw()
        .accounts({
          poolToken: new web3.PublicKey(tokenAddress),
          user: signer.publicKey
        })
        .signers([signer]).rpc();
      
      // Confirm transaction
      await program.provider.connection.confirmTransaction(txHash);
    });
  });
});
