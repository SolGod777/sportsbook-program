use anchor_lang::prelude::*;

/// accounts
#[account]
pub struct State {
    pub admin: Pubkey,
    pub vault: Pubkey,
    pub max_bet: u64,
    pub reward_multiplier: u16,
    pub delegated_admin_role: Pubkey,
}

impl State {
    pub const LEN: usize = 32 + 32 + 8 + 2 + 32; // pubkey
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
