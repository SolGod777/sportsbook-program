// errors
use anchor_lang::prelude::*;

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
    #[msg("MaxBetExceeded.")]
    MaxBetExceeded,
    #[msg("InvalidDelegate.")]
    InvalidDelegate,
}
