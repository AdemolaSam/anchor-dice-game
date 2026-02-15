import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AnchorDice2026 } from "../target/types/anchor_dice_2026";
import {
  Ed25519Program,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  sendAndConfirmTransaction,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
} from "@solana/web3.js";
import { BN } from "bn.js";
import { randomBytes } from "crypto";

describe("anchor-dice-game-q4-25", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.anchorDice2026 as Program<AnchorDice2026>;

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  let house = new Keypair();
  let player = new Keypair();
  let seed = new BN(randomBytes(16));

  let [vault, vaultBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), house.publicKey.toBuffer()],
    program.programId,
  );

  let [bet, betBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("bet"), vault.toBuffer(), seed.toBuffer("le", 16)],
    program.programId,
  );
  let signature: Uint8Array;

  it("Airdrop", async () => {
    await Promise.all(
      [house, player].map(async (key) => {
        const sig = await provider.connection.requestAirdrop(
          key.publicKey,
          10 * LAMPORTS_PER_SOL,
        );
        // wait for each airdrop to confirm
        const latestBlockHash = await provider.connection.getLatestBlockhash();
        await provider.connection.confirmTransaction({
          signature: sig,
          ...latestBlockHash,
        });
      }),
    );
  });

  it("Is initialized!", async () => {
    // Intialize house and vault
    const tx = await program.methods
      .initialize(new BN(LAMPORTS_PER_SOL).mul(new BN(4)))
      .accountsStrict({
        house: house.publicKey,
        vault,
        systemProgram: SystemProgram.programId,
      })
      .signers([house])
      .rpc()
      .then(confirmTx);
    console.log("Your transaction signature", tx);
  });

  it("Place a bet", async () => {
    let tx = await program.methods
      .placeBet(seed, 40, new BN(LAMPORTS_PER_SOL / 100))
      .accountsStrict({
        player: player.publicKey,
        house: house.publicKey,
        vault,
        bet,
        systemProgram: SystemProgram.programId,
      })
      .signers([player])
      .rpc()
      .then(confirmTx);

    console.log("Bet placed: ", tx);
  });

  it("Resolves bet", async () => {
    let account = await provider.connection.getAccountInfo(bet, "confirmed");
    let sig_ix = Ed25519Program.createInstructionWithPrivateKey({
      privateKey: house.secretKey,
      message: account.data.subarray(8),
    });

    const resolve_ix = await program.methods
      .resolveBet(Buffer.from(sig_ix.data.buffer.slice(16 + 32, 16 + 32 + 64)))
      .accountsStrict({
        player: player.publicKey,
        house: house.publicKey,
        vault,
        bet,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .signers([house])
      .instruction();

    const tx = new Transaction().add(sig_ix).add(resolve_ix);

    try {
      await sendAndConfirmTransaction(program.provider.connection, tx, [house]);
    } catch (error) {
      console.error(error);
      throw error;
    }
  });
});

const confirmTx = async (signature: string): Promise<string> => {
  const latestBlockHash = await anchor
    .getProvider()
    .connection.getLatestBlockhash();
  await anchor
    .getProvider()
    .connection.confirmTransaction(
      { signature, ...latestBlockHash },
      "confirmed",
    );

  return signature;
};
