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
});
