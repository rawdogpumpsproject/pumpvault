use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("HV1f42YJf82F7QEZequkhkwS1fw1yoNHrMgAqgHreExV");

#[program]
mod staking_pool {
    use super::*;

    const APY: f64 = 6.9; // Annual Percentage Yield
    const MONTHLY_RATE: f64 = APY / 12.0; // Monthly reward rate (approx. 0.575%)
    const MONTH_SECONDS: i64 = 30 * 24 * 60 * 60;

    pub fn initialize(ctx: Context<Initialize>, initial_amount: u64) -> Result<()> {
        require!(initial_amount > 0, CustomError::InvalidDepositAmount);

        let pool = &mut ctx.accounts.pool;
        pool.total_staked = 0;

        // Transfer the initial tokens from initializer to the pool
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.initializer_token_account.to_account_info(),
                    to: ctx.accounts.pool_token_account.to_account_info(),
                    authority: ctx.accounts.initializer.to_account_info(),
                },
            ),
            initial_amount,
        )?;

        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, CustomError::InvalidDepositAmount);

        let user_account = &mut ctx.accounts.user_account;

        // Initialize user account if this is the first time
        if user_account.amount_staked == 0 {
            user_account.owner = ctx.accounts.user.key();
        }
        user_account.amount_staked += amount;
        user_account.staked_at = Clock::get()?.unix_timestamp;

        // Perform token transfer from user to pool
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.pool_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;

        // Update pool's total staked amount
        let pool = &mut ctx.accounts.pool;
        pool.total_staked += amount;

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let clock = Clock::get()?.unix_timestamp;

        let user_account = &mut ctx.accounts.user_account;

        // Check if one month has passed since the last withdrawal
        if user_account.last_withdraw_at > 0 {
            require!(
                clock >= user_account.last_withdraw_at + MONTH_SECONDS,
                CustomError::WithdrawalLocked
            );
        }

        let months_staked = (clock - user_account.staked_at) / (MONTH_SECONDS);
        require!(months_staked > 0, CustomError::InsufficientWithdrawal);

        let monthly_rate = MONTHLY_RATE / 100.0;
        let total_rewards =
            (user_account.amount_staked as f64 * monthly_rate * months_staked as f64) as u64;

        let max_withdrawable = user_account
            .amount_staked
            .min(user_account.amount_staked * months_staked as u64 / 10)
            + total_rewards;
        require!(max_withdrawable > 0, CustomError::InsufficientWithdrawal);

        // Perform token transfer
        let cpi_accounts = Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.pool.to_account_info(),
        };
        let seeds = &[b"pool".as_ref(), &[ctx.bumps.pool]];
        let signer = &[&seeds[..]];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                signer,
            ),
            max_withdrawable,
        )?;

        // Update balances
        user_account.amount_staked -= max_withdrawable;
        ctx.accounts.pool.total_staked -= max_withdrawable;

        // Update last withdrawal timestamp
        user_account.last_withdraw_at = clock;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = initializer,
        space = 8 + std::mem::size_of::<Pool>(),
        seeds = [b"pool"],
        bump
    )]
    pub pool: Account<'info, Pool>, // The staking pool account

    #[account(seeds = [b"pool"], bump)]
    pub pool_authority: AccountInfo<'info>, // Pool authority PDA

    #[account(mut)]
    pub initializer: Signer<'info>, // The user initializing the pool

    #[account(mut,
        associated_token::mint = pool_token,
        associated_token::authority = initializer,
    )]
    pub initializer_token_account: Account<'info, TokenAccount>, // Initializer's token account

    #[account(init,
        payer = initializer,
        token::mint = pool_token,
        token::authority = pool,
        seeds = [pool.key().as_ref()],
        bump,
    )]
    pub pool_token_account: Account<'info, TokenAccount>, // Pool's token account

    #[account(mut)]
    pub pool_token: Account<'info, Mint>,

    pub token_program: Program<'info, Token>, // Token program for CPI

    pub system_program: Program<'info, System>, // System program for account creation
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(
        seeds = [b"pool"],
        bump
    )]
    pub pool: Account<'info, Pool>, // The staking pool account

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + std::mem::size_of::<UserAccount>(),
        seeds = [b"user_account", user.key().as_ref()],
        bump
    )]
    pub user_account: Account<'info, UserAccount>, // User's staking account

    #[account(mut)]
    pub user: Signer<'info>, // The user making the deposit

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>, // User's token account

    #[account(mut,
        associated_token::mint = pool_token,
        associated_token::authority = user,
    )]
    pub pool_token_account: Account<'info, TokenAccount>, // Pool's token account

    #[account(mut)]
    pub pool_token: Account<'info, Mint>,

    pub token_program: Program<'info, Token>, // Token program for CPI

    pub system_program: Program<'info, System>, // Required for account creation
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"user_account", user.key().as_ref()],
        bump
    )]
    pub user_account: Account<'info, UserAccount>,

    #[account(
        seeds = [b"pool"],
        bump
    )]
    pub pool: Account<'info, Pool>,

    #[account(mut,
        associated_token::mint = pool_token,
        associated_token::authority = user,
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub pool_token: Account<'info, Mint>,

    #[account(mut,
        associated_token::mint = pool_token,
        associated_token::authority = user,
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct Pool {
    pub total_staked: u64,
}

#[account]
pub struct UserAccount {
    pub owner: Pubkey,         // Owner of the account
    pub amount_staked: u64,    // Total staked tokens
    pub staked_at: i64,        // Timestamp of the initial stake
    pub last_withdraw_at: i64, // Timestamp of the last withdrawal
}

#[error_code]
pub enum CustomError {
    #[msg("Invalid initializer account.")]
    InvalidInitializerAccount,
    #[msg("Invalid pool token account.")]
    InvalidPoolTokenAccount,
    #[msg("Insufficient withdrawal amount.")]
    InsufficientWithdrawal,
    #[msg("Insufficient deposit amount.")]
    InvalidDepositAmount,
    #[msg("Withdrawals are locked for one month from the last withdrawal.")]
    WithdrawalLocked,
}
