use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, Mint, TokenAccount, TokenInterface, TransferChecked
};
declare_id!("J7dkoQWFE736V1BfQgfiiqXe9pHH69gsEghbUoY4sb5y");

#[program]
pub mod sportsbook {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, admin: Pubkey) -> Result<()> {
        ctx.accounts.state.admin = admin;
        Ok(())
    }

    pub fn register_bet(ctx: Context<RegisterBet>, bet_id: u64) -> Result<()> {
        let bet = &mut ctx.accounts.bet;
        bet.id = bet_id;
        bet.open = true;
        bet.side = 0;
        Ok(())
    }

    pub fn set_winner(ctx: Context<SetWinner>, bet_id: u64, side: u8) -> Result<()> {
        let bet = &mut ctx.accounts.bet;
        require!(ctx.accounts.admin.key() == ctx.accounts.state.admin, ErrorCode::Unauthorized);
        require!(bet.open, ErrorCode::BetClosed);

        bet.open = false;
        bet.side = side;
        bet.total_pot = 0;
        Ok(())
    }

    pub fn place_bet(ctx: Context<BetHandler>, bet_id: u64, amount: u64, side: u8) -> Result<()> {
        let bet = &mut ctx.accounts.bet;
        require!(bet.open, ErrorCode::BetClosed);
        require!(side == 0 || side == 1, ErrorCode::InvalidSide);

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        token_interface::transfer_checked(cpi_context, amount, 6)?;

        let user_bet = &mut ctx.accounts.user_bet;
        user_bet.amount = amount;
        user_bet.bet = bet.key();

        bet.total_pot += amount;
        Ok(())
    }

    pub fn claim_winnings(ctx: Context<BetHandler>, bet_id: u64) -> Result<()> {
        let bet = &mut ctx.accounts.bet;
        require!(!bet.open, ErrorCode::BetOpen);

        let winnings = ctx.accounts.user_bet.amount;

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        token_interface::transfer_checked(cpi_context, winnings, 6)?;

        let user_bet = &mut ctx.accounts.user_bet;
        user_bet.amount = 0;

        Ok(())
    }


}


// contexts
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = payer, space = 8 + State::LEN, seeds = [b"state"], bump)]
    pub state: Account<'info, State>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(bet_id: u64)]
pub struct RegisterBet<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + Bet::LEN,
        seeds = [b"bet", bet_id.to_le_bytes().as_ref()],
        bump
    )]
    pub bet: Account<'info, Bet>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(bet_id: u64)]
pub struct SetWinner<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,

    #[account(
        mut,
        seeds = [b"bet", bet_id.to_le_bytes().as_ref()],
        bump
    )]
    pub bet: Account<'info, Bet>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(bet_id: u64)]
pub struct BetHandler<'info> {
    /// CHECK: Validated by transfer_checked CPI
    #[account(mut)]
    pub user_token_account: AccountInfo<'info>,

    /// CHECK: Mint passed to transfer_checked CPI
    pub mint: AccountInfo<'info>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,

    #[account(
        mut,
        seeds = [b"bet", bet_id.to_le_bytes().as_ref()],
        bump
    )]
    pub bet: Account<'info, Bet>,

    #[account(
        init,
        payer = user,
        space = 8 + UserBet::LEN,
        seeds = [b"user_bet", bet_id.to_le_bytes().as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_bet: Account<'info, UserBet>,

/// CHECK: Vault PDA token account initialized for bet, validated by CPI
#[account(
mut
)]
pub vault: AccountInfo<'info>,

    pub system_program: Program<'info, System>,

}


/// accounts
#[account]
pub struct State {
    pub admin: Pubkey,
}

impl State {
    pub const LEN: usize = 32; // pubkey
}

#[account]
pub struct Bet {
    pub id: u64,
    pub open: bool,
    pub side: u8,
    pub total_pot: u64
}

impl Bet {
    pub const LEN: usize = 8 + 1 + 1 + 8; 
}

#[account]
pub struct UserBet {
    pub user: Pubkey,
    pub bet: Pubkey,
    pub amount: u64,
    pub side: u8, // 0 = home, 1 = away
}

impl UserBet {
    pub const LEN: usize = 32 + 32 + 8 + 1;
}

// errors
#[error_code]
pub enum ErrorCode {
    #[msg("Only the admin can perform this action.")]
    Unauthorized,
    #[msg("Bet is closed.")]
    BetClosed,
    #[msg("Bet is open.")]
    BetOpen,
    #[msg("Invalid side. Must be 0 (home) or 1 (away).")]
    InvalidSide,
}
