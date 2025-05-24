use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use anchor_spl::token_2022::initialize_account3;
use anchor_spl::token_interface::{self, TokenInterface, TransferChecked};

declare_id!("J7dkoQWFE736V1BfQgfiiqXe9pHH69gsEghbUoY4sb5y");
const SFLOW_MINT_LEN: usize = 278;
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

        let cpi_context = CpiContext::new_with_signer(
            cpi_program,
            cpi_accounts,
            signer_seeds,
        );
        
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

/// accounts
#[account]
pub struct State {
    pub admin: Pubkey,
    pub vault: Pubkey,
}

impl State {
    pub const LEN: usize = 32 + 32; // pubkey
}

#[account]
pub struct Bet {
    pub id: u64,
    pub open: bool,
    pub side: u8,
    pub total_pot: u64,
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
    #[msg("LostBet")]
    LostBet,
    #[msg("Invalid vault.")]
    InvalidVault,
    #[msg("MathOverflow.")]
    MathOverflow,
}
