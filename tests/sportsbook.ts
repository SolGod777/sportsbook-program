import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { assert } from "chai";
import { Sportsbook } from "../target/types/sportsbook";
import {
  createAccount,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  createMint,
  ExtensionType,
  getAccount,
  getMintLen,
  mintTo,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";

describe("sportsbook", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Sportsbook as Program<Sportsbook>;

  const admin = provider.wallet.publicKey;

  let statePda: PublicKey;
  let stateBump: number;

  let betPda: PublicKey;
  let betBump: number;

  const betId = new anchor.BN(42);

  let vaultPda: PublicKey;
  let vaultBump: number;
  let mint: PublicKey;
  let userTokenAccount: PublicKey;
  let userBetPda: PublicKey;
  let user = Keypair.generate();
  let adminTokenAccount: PublicKey;

  it("Initializes the state", async () => {
    // Airdrop SOL to the test user
    await provider.connection.requestAirdrop(user.publicKey, 2e9);
    await new Promise((r) => setTimeout(r, 2000));

    const mintKeypair = Keypair.generate();

    const mintLen = getMintLen([ExtensionType.TransferFeeConfig]);
    console.log(mintLen);
    const lamports =
      await provider.connection.getMinimumBalanceForRentExemption(mintLen);

    const createMintAccountIx = SystemProgram.createAccount({
      fromPubkey: user.publicKey,
      newAccountPubkey: mintKeypair.publicKey,
      space: mintLen,
      lamports,
      programId: TOKEN_2022_PROGRAM_ID,
    });

    const initFeeIx = createInitializeTransferFeeConfigInstruction(
      mintKeypair.publicKey,
      user.publicKey, // config authority
      null, // withdraw withheld authority
      500, // 5% fee in basis points
      BigInt(1_000_000),
      TOKEN_2022_PROGRAM_ID
    );

    const initMintIx = createInitializeMintInstruction(
      mintKeypair.publicKey,
      6, // decimals
      user.publicKey, // mint authority
      null,
      TOKEN_2022_PROGRAM_ID
    );

    const tx = new anchor.web3.Transaction().add(
      createMintAccountIx,
      initFeeIx,
      initMintIx
    );
    await provider.sendAndConfirm(tx, [user, mintKeypair]);

    mint = mintKeypair.publicKey;
    adminTokenAccount = await createAccount(
      provider.connection,
      user,
      mint,
      admin,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    [statePda, stateBump] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("state")],
      program.programId
    );

    [vaultPda, vaultBump] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault")],
      program.programId
    );

    await program.methods
      .initialize(admin)
      .accounts({
        state: statePda,
        payer: admin,
        vault: vaultPda,
        mint,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const vaultAccount = await getAccount(
      provider.connection,
      vaultPda,
      null,
      TOKEN_2022_PROGRAM_ID
    );

    const vaultBalance = vaultAccount.amount;
    console.log("Vault balance:", vaultBalance.toString());
    const state = await program.account.state.fetch(statePda);
    console.log("state", state);
    assert.ok(state.admin.equals(admin));
  });

  it("Registers a new bet", async () => {
    [betPda, betBump] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), betId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );

    await program.methods
      .registerBet(betId)
      .accounts({
        admin,
        bet: betPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const bet = await program.account.bet.fetch(betPda);
    assert.ok(bet.id.toNumber() === betId.toNumber());
    assert.equal(bet.open, true);
  });
  it("Places a bet", async () => {
    userTokenAccount = await createAccount(
      provider.connection,
      user,
      mint,
      user.publicKey,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    // Mint tokens to user
    await mintTo(
      provider.connection,
      user,
      mint,
      adminTokenAccount,
      user,
      1000_000_000,
      [],
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    await mintTo(
      provider.connection,
      user,
      mint,
      userTokenAccount,
      user,
      1_000_000,
      [],
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    [userBetPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("user_bet"),
        betId.toArrayLike(Buffer, "le", 8),
        user.publicKey.toBuffer(),
      ],
      program.programId
    );

    const betAmount = new anchor.BN(400_000);
    const amountAfterFee = 400_000 - Math.floor(400_000 * 0.05);

    await program.methods
      .placeBet(betId, betAmount, 0)
      .accounts({
        user: user.publicKey,
        bet: betPda,
        userBet: userBetPda,
        userTokenAccount: userTokenAccount,
        vault: vaultPda,
        mint,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    const bet = await program.account.bet.fetch(betPda);
    assert.equal(bet.totalPot.toNumber(), amountAfterFee);

    const vaultAccount = await getAccount(
      provider.connection,
      vaultPda,
      null,
      TOKEN_2022_PROGRAM_ID
    );

    const vaultBalance = vaultAccount.amount;
    console.log("Vault balance:", vaultBalance.toString());
    assert.equal(vaultBalance.toString(), amountAfterFee.toString());
  });

  it("Claims winnings with 5% fee", async () => {
    await program.methods
      .fundVault(new anchor.BN(800_000))
      .accounts({
        admin,
        adminTokenAccount,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        mint,
        vault: vaultPda,
      })
      .rpc();
    const vaultAccount = await getAccount(
      provider.connection,
      vaultPda,
      null,
      TOKEN_2022_PROGRAM_ID
    );

    const vaultBalance = vaultAccount.amount;
    console.log("Vault balance:", vaultBalance.toString());
    // Fetch user's token account balance before
    let userBefore = await getAccount(
      provider.connection,
      userTokenAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const balanceBefore = Number(userBefore.amount);

    // Simulate game result
    await program.methods
      .setWinner(betId, 0)
      .accounts({
        admin,
        bet: betPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Claim winnings
    await program.methods
      .claimWinnings(betId)
      .accounts({
        user: user.publicKey,
        bet: betPda,
        userBet: userBetPda,
        userTokenAccount,
        vault: vaultPda,
        vaultAuthority: vaultPda,
        mint,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        state: statePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    // Fetch user's token account balance after
    let userAfter = await getAccount(
      provider.connection,
      userTokenAccount,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const balanceAfter = Number(userAfter.amount);

    const betAmount = 400_000;
    const netPlaced = Math.floor(betAmount * 0.95); // 380,000
    const expectedWinnings = Math.floor(netPlaced * 2.05); // 779,000
    const expectedReceived = Math.floor(expectedWinnings * 0.95); // 740,050

    const received = balanceAfter - balanceBefore;
    console.log("Winnings claimed (net after fee):", received);

    assert.ok(
      Math.abs(received - expectedReceived) <= 1,
      `Expected ${expectedReceived}, but got ${received}`
    );
  });

  it("Sets a side in a bet", async () => {
    const betIdTemp = new anchor.BN(1);
    let betPdaTemp: PublicKey;
    let betBumpTemp: number;
    [betPdaTemp, betBumpTemp] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), betIdTemp.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
    console.log(betPda);

    await program.methods
      .registerBet(betIdTemp)
      .accounts({
        admin,
        bet: betPdaTemp,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    await program.methods
      .setWinner(betIdTemp, 1)
      .accounts({
        admin,
        bet: betPdaTemp,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const bet = await program.account.bet.fetch(betPdaTemp);
    assert.ok(bet.id.toNumber() === betIdTemp.toNumber());
    assert.equal(bet.open, false);
    assert.equal(bet.side, 1);
  });
  it("Delegates new admin role", async () => {
    const newAdmin = Keypair.generate();
    await provider.connection.requestAirdrop(newAdmin.publicKey, 2e9);
    await new Promise((r) => setTimeout(r, 2000));

    await program.methods
      .delegateAdmin(newAdmin.publicKey)
      .accounts({
        admin,
        state: statePda,
      })
      .rpc();
    let state = await program.account.state.fetch(statePda);

    assert.equal(
      state.delegatedAdminRole.toBase58(),
      newAdmin.publicKey.toBase58()
    );

    await program.methods
      .acceptAdminRole()
      .accounts({
        state: statePda,
        newAdmin: newAdmin.publicKey,
      })
      .signers([newAdmin])
      .rpc();

    state = await program.account.state.fetch(statePda);

    assert.equal(state.admin.toBase58(), newAdmin.publicKey.toBase58());
  });
  it("Fails to delegate admin if caller is not admin", async () => {
    const fakeAdmin = Keypair.generate();
    await provider.connection.requestAirdrop(fakeAdmin.publicKey, 2e9);
    await new Promise((r) => setTimeout(r, 2000));

    const anotherUser = Keypair.generate();

    try {
      await program.methods
        .delegateAdmin(anotherUser.publicKey)
        .accounts({
          state: statePda,
          admin: fakeAdmin.publicKey,
        })
        .signers([fakeAdmin]) // Not the real admin
        .rpc();

      assert.fail("Expected Unauthorized error, but transaction succeeded");
    } catch (err: any) {
      // Anchor error code check
      const anchorError = err.error || err;
      const errorCode = anchorError?.errorCode?.code;
      const errorMsg = anchorError?.errorMessage || anchorError.toString();

      console.log("Caught error:", errorCode, errorMsg);
      assert.ok(
        errorCode === "Unauthorized" || errorMsg.includes("Unauthorized"),
        `Expected Unauthorized error, got: ${errorMsg}`
      );
    }
  });
});
