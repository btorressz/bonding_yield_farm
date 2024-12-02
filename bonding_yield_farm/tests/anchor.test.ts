import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { Keypair, PublicKey, SystemProgram } from '@solana/web3.js';
import BN from 'bn.js';
import assert from 'assert';

describe("Bonding Yield Farming Tests", () => {
  let poolKp: Keypair;
  let userKp: Keypair;
  let farmMintKp: Keypair;
  let userTokenAccount: Keypair;

  const rewardCoefficient = new BN(100); 
  const maxDepositPerUser = new BN(1000);
  const totalMaxLiquidity = new BN(10000);

  before(async () => {
    // Initialize keypairs
    poolKp = Keypair.generate();
    userKp = Keypair.generate();
    farmMintKp = Keypair.generate();
    userTokenAccount = Keypair.generate();

    // Fund accounts with SOL for transaction fees
    const connection = pg.connection;
    await connection.requestAirdrop(userKp.publicKey, web3.LAMPORTS_PER_SOL);
    await connection.requestAirdrop(userTokenAccount.publicKey, web3.LAMPORTS_PER_SOL);
  });

  it("initializes the pool", async () => {
    const txHash = await pg.program.methods
      .initializePool(
        farmMintKp.publicKey,
        rewardCoefficient,
        maxDepositPerUser,
        totalMaxLiquidity
      )
      .accounts({
        pool: poolKp.publicKey,
        authority: pg.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([poolKp])
      .rpc();

    console.log(`Initialized pool: ${txHash}`);
    const pool = await pg.program.account.pool.fetch(poolKp.publicKey);
    console.log("Pool State:", pool);
    assert.equal(pool.rewardCoefficient.toString(), rewardCoefficient.toString());
  });

  it("stakes tokens", async () => {
    const stakeAmount = new BN(500);
    const lockupPeriod = new BN(60 * 60 * 24 * 7); // 7 days

    const txHash = await pg.program.methods
      .stake(stakeAmount, false, lockupPeriod)
      .accounts({
        pool: poolKp.publicKey,
        userPosition: userKp.publicKey,
        userLiquidity: userTokenAccount.publicKey,
        farmMint: farmMintKp.publicKey,
        userFarmToken: userTokenAccount.publicKey,
        farmMintAuthority: farmMintKp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        user: pg.wallet.publicKey,
      })
      .signers([userKp])
      .rpc();

    console.log(`Staked tokens: ${txHash}`);
    const userPosition = await pg.program.account.stakedPosition.fetch(userKp.publicKey);
    console.log("User Position:", userPosition);
    assert.equal(userPosition.amount.toString(), stakeAmount.toString());
  });

  it("withdraws tokens after lockup", async () => {
    const withdrawAmount = new BN(200);

    // Simulate withdrawal
    const txHash = await pg.program.methods
      .withdraw(withdrawAmount)
      .accounts({
        pool: poolKp.publicKey,
        userPosition: userKp.publicKey,
        userLiquidity: userTokenAccount.publicKey,
        poolLiquidity: poolKp.publicKey,
        treasuryAccount: userTokenAccount.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        user: pg.wallet.publicKey,
      })
      .signers([userKp])
      .rpc();

    console.log(`Withdrawn tokens: ${txHash}`);
    const userPosition = await pg.program.account.stakedPosition.fetch(
      userKp.publicKey
    );
    console.log("User Position After Withdrawal:", userPosition);
    assert.equal(
      userPosition.amount.toString(),
      new BN(500 - 200).toString()
    );
  });
});
