use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, TokenInterface, TransferChecked};

use crate::{error::ErrorCode, state::*};

// contexts
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = payer, space = 8 + State::LEN, seeds = [b"state"], bump)]
    pub state: Account<'info, State>,

    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Vault token account PDA will be created via CPI
    #[account(mut)]
    pub vault: AccountInfo<'info>,

    /// CHECK: PDA that will be the authority of the vault
    #[account(seeds = [b"vault"], bump)]
    pub vault_authority: AccountInfo<'info>,

    /// CHECK: Mint passed to transfer_checked CPI
    pub mint: AccountInfo<'info>,
    /// CHECK: This is a Token-2022 program, manually verified
    pub token_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetSettings<'info> {
    #[account(mut, seeds = [b"state"], bump)]
    pub state: Account<'info, State>,

    #[account(
        constraint = admin.key() == state.admin @ ErrorCode::Unauthorized
    )]    
    pub admin: Signer<'info>,

}

#[derive(Accounts)]
pub struct AcceptAdminRole<'info> {
    #[account(mut, seeds = [b"state"], bump)]
    pub state: Account<'info, State>,

    #[account(
        constraint = new_admin.key() == state.delegated_admin_role @ ErrorCode::Unauthorized
    )]    
    pub new_admin: Signer<'info>,

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
pub struct FundVault<'info> {
    #[account(
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,

    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: Validated by transfer_checked CPI
    #[account(mut)]
    pub admin_token_account: AccountInfo<'info>,

    /// CHECK: Vault token account PDA will be created via CPI
    #[account(mut)]
    pub vault: AccountInfo<'info>,

    /// CHECK: Mint passed to transfer_checked CPI
    pub mint: AccountInfo<'info>,
    /// CHECK: This is a Token-2022 program, manually verified
    pub token_program: AccountInfo<'info>,
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
pub struct PlaceBet<'info> {
    /// CHECK: Validated by transfer_checked CPI
    #[account(mut)]
    pub user_token_account: AccountInfo<'info>,

    /// CHECK: Mint passed to transfer_checked CPI
    pub mint: AccountInfo<'info>,

    #[account(
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,

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

    /// CHECK: Validated by transfer_checked CPI
    #[account(mut)]
    pub vault: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(bet_id: u64)]
pub struct ClaimWinnings<'info> {
    /// CHECK: Validated by transfer_checked CPI
    #[account(mut)]
    pub user_token_account: AccountInfo<'info>,

    /// CHECK: Mint passed to transfer_checked CPI
    pub mint: AccountInfo<'info>,

    #[account(
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,

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
        seeds = [b"user_bet", bet_id.to_le_bytes().as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_bet: Account<'info, UserBet>,

    /// CHECK: Validated by transfer_checked CPI
    #[account(mut)]
    pub vault: AccountInfo<'info>,

    /// CHECK: PDA authority for the vault
    #[account(seeds = [b"vault"], bump)]
    pub vault_authority: AccountInfo<'info>,    

    pub system_program: Program<'info, System>,
}

