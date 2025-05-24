import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { assert } from "chai";
import { Sportsbook } from "../target/types/sportsbook";
import {
  createAccount,
  createMint,
  getAccount,
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

    // Create SPL2022 mint & accounts
    mint = await createMint(
      provider.connection,
      user,
      user.publicKey,
      null,
      6,
      mintKeypair,
      null,
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

    await program.methods
      .placeBet(betId, new anchor.BN(400000), 0)
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
    assert.equal(bet.totalPot.toNumber(), 400000);

    const vaultAccount = await getAccount(
      provider.connection,
      vaultPda,
      null,
      TOKEN_2022_PROGRAM_ID
    );

    const vaultBalance = vaultAccount.amount;
    console.log("Vault balance:", vaultBalance.toString());
    assert.equal(vaultBalance.toString(), "400000");
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
