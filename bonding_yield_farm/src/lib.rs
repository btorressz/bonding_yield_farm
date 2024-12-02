use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

declare_id!("AbNTrViZTe1iAXzSWPEfWqKyNhFF19MxrDss2ReNMau5");

#[program]
mod bonding_yield_farm {
    use super::*;

    // Initialize the pool
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        token_mint: Pubkey,
        reward_coefficient: u64,
        max_deposit_per_user: u64,
        total_max_liquidity: u64,
    ) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.token_mint = token_mint;
        pool.total_liquidity = 0;
        pool.reward_coefficient = reward_coefficient;
        pool.fee_rate = 2; // Default fee rate: 2%
        pool.last_update = Clock::get()?.unix_timestamp;
        pool.top_staker = Pubkey::default(); // Initialize empty leaderboard
        pool.top_staker_amount = 0; // Initialize top staker amount
        pool.total_rewards_distributed = 0; // Initialize rewards tracking
        pool.max_deposit_per_user = max_deposit_per_user;
        pool.total_max_liquidity = total_max_liquidity;
        pool.is_paused = false; // Not paused initially

        emit!(PoolInitializedEvent {
            pool: pool.key(),
            token_mint,
            reward_coefficient,
        });

        Ok(())
    }

    // Stake liquidity
    pub fn stake(
        ctx: Context<Stake>,
        amount: u64,
        auto_compound: bool,
        lockup_period: Option<u64>,
    ) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        let user_position = &mut ctx.accounts.user_position;

        require!(!pool.is_paused, ErrorCode::PoolPaused);

        // Check total pool liquidity limit
        require!(
            pool.total_liquidity + amount <= pool.total_max_liquidity,
            ErrorCode::PoolLiquidityExceeded
        );

        // Check individual user deposit limit
        require!(
            user_position.amount + amount <= pool.max_deposit_per_user,
            ErrorCode::UserDepositLimitExceeded
        );

        // Update user's position
        user_position.amount += amount;
        user_position.stake_time = Clock::get()?.unix_timestamp;

        // Set optional lockup period
        if let Some(duration) = lockup_period {
            user_position.unlock_time = Clock::get()?.unix_timestamp + duration as i64;
        }

        // Update pool state
        pool.total_liquidity += amount;

        // Calculate boosted rewards
        let reward = calculate_boosted_reward(
            user_position.amount,
            user_position.stake_time,
            pool.last_update,
            pool.reward_coefficient,
        );

        if auto_compound {
            user_position.amount += reward; // Auto-compound reward into staked amount
        } else {
            mint_tokens(
                &ctx.accounts.token_program,
                &ctx.accounts.farm_mint,
                &ctx.accounts.user_farm_token,
                &ctx.accounts.farm_mint_authority,
                reward,
            )?;
        }

        // Track total rewards
        pool.total_rewards_distributed += reward;

        // Update leaderboard
        if user_position.amount > pool.top_staker_amount {
            pool.top_staker = ctx.accounts.user.key();
            pool.top_staker_amount = user_position.amount;
        }

        emit!(StakeEvent {
            user: ctx.accounts.user.key(),
            amount,
            rewards: reward,
        });

        Ok(())
    }

    // Withdraw liquidity
    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        let user_position = &mut ctx.accounts.user_position;
        let pool = &mut ctx.accounts.pool;

        require!(!pool.is_paused, ErrorCode::PoolPaused);

        // Validate lockup period
        let current_time = Clock::get()?.unix_timestamp;
        require!(current_time >= user_position.unlock_time, ErrorCode::StillLocked);

        // Validate withdrawal
        require!(user_position.amount >= amount, ErrorCode::InsufficientFunds);

        // Apply fee
        let fee = amount * pool.fee_rate as u64 / 100; // Convert fee_rate to u64
        let net_amount = amount - fee;

        // Update user's position and pool state
        user_position.amount -= amount;
        pool.total_liquidity -= amount;

        // Transfer fee to treasury
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.pool_liquidity.to_account_info(),
                    to: ctx.accounts.treasury_account.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
            ),
            fee,
        )?;

        // Transfer net amount to user
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.pool_liquidity.to_account_info(),
                    to: ctx.accounts.user_liquidity.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
            ),
            net_amount,
        )?;

        emit!(WithdrawEvent {
            user: ctx.accounts.user.key(),
            amount,
            fee_amount: fee,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    // Admin toggle for pause/unpause
    pub fn toggle_pause(ctx: Context<AdminContext>) -> Result<()> {
        let pool = &mut ctx.accounts.pool;

        // Ensure only the admin can toggle
        require!(ctx.accounts.admin.key() == pool.admin, ErrorCode::Unauthorized);

        pool.is_paused = !pool.is_paused;
        Ok(())
    }
}

// Helper functions
fn calculate_boosted_reward(
    amount: u64,
    stake_time: i64,
    last_update: i64,
    reward_coefficient: u64,
) -> u64 {
    let elapsed_time = (stake_time - last_update).max(0) as u64;
    let time_multiplier = 100 + (elapsed_time / 86400); // +1% per day
    let amount_multiplier = 100 + (amount / 1000); // Bonus for larger stakes

    amount * time_multiplier * amount_multiplier * reward_coefficient / 10000
}

fn mint_tokens<'info>(
    token_program: &Program<'info, Token>,
    farm_mint: &Account<'info, Mint>,
    to: &Account<'info, TokenAccount>,
    authority: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let (farm_mint_authority, farm_mint_bump) =
        Pubkey::find_program_address(&[b"farm-mint"], &token_program.key());
    let seeds = &[b"farm-mint".as_ref(), &[farm_mint_bump]];
    let signer = &[&seeds[..]];

    require!(*authority.key == farm_mint_authority, ErrorCode::InvalidAuthority);

    token::mint_to(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            token::MintTo {
                mint: farm_mint.to_account_info(),
                to: to.to_account_info(),
                authority: authority.clone(),
            },
            signer,
        ),
        amount,
    )?;
    Ok(())
}

// Events
#[event]
pub struct PoolInitializedEvent {
    pub pool: Pubkey,
    pub token_mint: Pubkey,
    pub reward_coefficient: u64,
}

#[event]
pub struct StakeEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub rewards: u64,
}

#[event]
pub struct WithdrawEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub fee_amount: u64,
    pub timestamp: i64,
}

// Accounts
#[account]
pub struct Pool {
    pub admin: Pubkey,
    pub token_mint: Pubkey,
    pub total_liquidity: u64,
    pub reward_coefficient: u64,
    pub fee_rate: u64,
    pub last_update: i64,
    pub top_staker: Pubkey,
    pub top_staker_amount: u64,
    pub total_rewards_distributed: u64,
    pub max_deposit_per_user: u64,
    pub total_max_liquidity: u64,
    pub is_paused: bool,
}

#[account]
pub struct StakedPosition {
    pub amount: u64,
    pub stake_time: i64,
    pub unlock_time: i64,
    pub multiplier: u64,
}

// Contexts
#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(init, payer = authority, space = 8 + std::mem::size_of::<Pool>())]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user_position: Account<'info, StakedPosition>,
    #[account(mut)]
    pub user_liquidity: Account<'info, TokenAccount>,
    #[account(mut)]
    pub farm_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_farm_token: Account<'info, TokenAccount>,
    pub farm_mint_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user_position: Account<'info, StakedPosition>,
    #[account(mut)]
    pub user_liquidity: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_liquidity: Account<'info, TokenAccount>,
    #[account(mut)]
    pub treasury_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub user: Signer<'info>, // Fixed: Added user signer
}

#[derive(Accounts)]
pub struct AdminContext<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    pub admin: Signer<'info>,
}

// Errors
#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient funds to withdraw.")]
    InsufficientFunds,
    #[msg("Invalid farm mint authority.")]
    InvalidAuthority,
    #[msg("Your stake is still locked.")]
    StillLocked,
    #[msg("Pool liquidity limit exceeded.")]
    PoolLiquidityExceeded,
    #[msg("User deposit limit exceeded.")]
    UserDepositLimitExceeded,
    #[msg("Pool is currently paused.")]
    PoolPaused,
    #[msg("Unauthorized operation.")]
    Unauthorized,
}
