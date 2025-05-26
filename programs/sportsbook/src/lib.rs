use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use anchor_spl::token_2022::initialize_account3;
use anchor_spl::token_interface::{self, TokenInterface, TransferChecked};
mod error;
use error::ErrorCode;

mod consts;
use consts::*;

mod state;

mod contexts;
use contexts::*;

declare_id!("J7dkoQWFE736V1BfQgfiiqXe9pHH69gsEghbUoY4sb5y");
#[program]
pub mod sportsbook {

    use anchor_spl::token_2022::InitializeAccount3;

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, admin: Pubkey) -> Result<()> {
        let vault_key = ctx.accounts.vault.key();

        // Allocate space and fund the vault account manually
        let rent = Rent::get()?.minimum_balance(SFLOW_MINT_LEN);

        invoke_signed(
            &system_instruction::create_account(
                ctx.accounts.payer.key,
                &vault_key,
                rent,
                SFLOW_MINT_LEN as u64,
                ctx.accounts.token_program.key,
            ),
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[&[b"vault", &[ctx.bumps.vault_authority]]],
        )?;

        let vault_bump = ctx.bumps.vault_authority;
        let vault_seed_prefix = b"vault";
        let vault_seed_bump: &[u8] = &[vault_bump];
        let vault_seeds: &[&[u8]] = &[vault_seed_prefix, vault_seed_bump];
        let signer_seeds: &[&[&[u8]]] = &[vault_seeds];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            InitializeAccount3 {
                account: ctx.accounts.vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer_seeds,
        );

        initialize_account3(cpi_ctx)?;

        ctx.accounts.state.admin = admin;
        ctx.accounts.state.vault = vault_key;
        ctx.accounts.state.reward_multiplier = 205;
        ctx.accounts.state.max_bet = 1_000_000 * SFLOW_DECIMALS;
        Ok(())
    }

    pub fn register_bet(ctx: Context<RegisterBet>, bet_id: u64) -> Result<()> {
        let bet = &mut ctx.accounts.bet;
        bet.id = bet_id;
        bet.open = true;
        bet.side = 0;
        Ok(())
    }

    pub fn fund_vault(ctx: Context<FundVault>, amount: u64) -> Result<()> {
        let state = &ctx.accounts.state;
        let vault = &mut ctx.accounts.vault;
        let admin = &mut ctx.accounts.admin;

        require!(state.admin == admin.key(), ErrorCode::Unauthorized);
        require!(vault.key() == state.vault, ErrorCode::InvalidVault);

        let vault_before = {
            let data = vault.try_borrow_data()?;
            u64::from_le_bytes(data[64..72].try_into().unwrap())
        };

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.admin_token_account.to_account_info(),
            to: vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.admin.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        token_interface::transfer_checked(cpi_context, amount, 6)?;

        let vault_after = {
            let data = ctx.accounts.vault.try_borrow_data()?;
            u64::from_le_bytes(data[64..72].try_into().unwrap())
        };

        let _received = vault_after
            .checked_sub(vault_before)
            .ok_or_else(|| error!(ErrorCode::MathOverflow))?;

        Ok(())
    }

    pub fn set_winner(ctx: Context<SetWinner>, bet_id: u64, side: u8) -> Result<()> {
        let bet = &mut ctx.accounts.bet;
        require!(
            ctx.accounts.admin.key() == ctx.accounts.state.admin,
            ErrorCode::Unauthorized
        );
        require!(bet.open, ErrorCode::BetClosed);

        bet.open = false;
        bet.side = side;
        bet.total_pot = 0;
        Ok(())
    }

    pub fn place_bet(ctx: Context<PlaceBet>, bet_id: u64, amount: u64, side: u8) -> Result<()> {
        let state = &ctx.accounts.state;
        let bet = &mut ctx.accounts.bet;
        let vault = &mut ctx.accounts.vault;
        require!(bet.open, ErrorCode::BetClosed);
        require!(side == 0 || side == 1, ErrorCode::InvalidSide);
        require!(vault.key() == state.vault, ErrorCode::InvalidVault);
        require!(amount <= state.max_bet, ErrorCode::MaxBetExceeded);

        let vault_before = {
            let data = vault.try_borrow_data()?;
            u64::from_le_bytes(data[64..72].try_into().unwrap())
        };

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        token_interface::transfer_checked(cpi_context, amount, 6)?;

        let vault_after = {
            let data = ctx.accounts.vault.try_borrow_data()?;
            u64::from_le_bytes(data[64..72].try_into().unwrap())
        };

        let received = vault_after
            .checked_sub(vault_before)
            .ok_or_else(|| error!(ErrorCode::MathOverflow))?;
        let user_bet = &mut ctx.accounts.user_bet;
        user_bet.amount = received;
        user_bet.bet = bet.key();

        bet.total_pot += received;
        Ok(())
    }

    pub fn claim_winnings(ctx: Context<ClaimWinnings>, bet_id: u64) -> Result<()> {
        let state = &ctx.accounts.state;
        let bet = &mut ctx.accounts.bet;
        let vault = &ctx.accounts.vault;
        let user_bet = &ctx.accounts.user_bet;

        require!(!bet.open, ErrorCode::BetOpen);
        require!(vault.key() == state.vault, ErrorCode::InvalidVault);
        require!(bet.side == user_bet.side, ErrorCode::LostBet);

        let base = user_bet.amount;
        let winnings = (base * 205) / 100;
        msg!(&winnings.to_string());

        let cpi_accounts = TransferChecked {
            from: vault.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let signer_seeds: &[&[&[u8]]] = &[&[b"vault", &[ctx.bumps.vault_authority]]];

        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        token_interface::transfer_checked(cpi_context, winnings, 6)?;

        let user_bet = &mut ctx.accounts.user_bet;
        user_bet.amount = 0;

        Ok(())
    }

    pub fn set_max_bet(ctx: Context<SetSettings>, max_bet: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;

        state.max_bet = max_bet;

        Ok(())
    }

    pub fn set_reward_multiplier(ctx: Context<SetSettings>, reward_multiplier: u16) -> Result<()> {
        let state = &mut ctx.accounts.state;

        state.reward_multiplier = reward_multiplier;

        Ok(())
    }

    pub fn delegate_admin(ctx: Context<SetSettings>, new_admin: Pubkey) -> Result<()> {
        let state = &mut ctx.accounts.state;
        require!(new_admin.key() != state.admin, ErrorCode::InvalidDelegate);
        state.delegated_admin_role = new_admin.key();

        Ok(())
    }

    pub fn accept_admin_role(ctx: Context<AcceptAdminRole>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let new_admin = &ctx.accounts.new_admin;

        state.admin = new_admin.key();
        msg!("Admin changed");

        Ok(())
    }
}
