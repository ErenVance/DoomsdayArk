use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Token, TokenAccount, Transfer};

pub fn transfer_from_player_to_vault<'info>(
    authority: &Signer<'info>,
    token_account: &Account<'info, TokenAccount>,
    token_vault: &Account<'info, TokenAccount>,
    token_program: &Program<'info, Token>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            token_program.to_account_info(),
            Transfer {
                from: token_account.to_account_info(),
                to: token_vault.to_account_info(),
                authority: authority.to_account_info(),
            },
        ),
        amount,
    )
}

pub fn redeem_vouchers<'info, T: AccountSerialize + AccountDeserialize + Clone>(
    authority: &Account<'info, T>,
    voucher_vault: &Account<'info, TokenAccount>,
    token_account: &Account<'info, TokenAccount>,
    token_program: &Program<'info, Token>,
    amount: u64,
    seeds: &[&[u8]],
) -> Result<()> {
    transfer(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            Transfer {
                from: voucher_vault.to_account_info(),
                to: token_account.to_account_info(),
                authority: authority.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )
}

pub fn transfer_from_token_vault_to_token_account<
    'info,
    T: AccountSerialize + AccountDeserialize + Clone,
>(
    authority: &Account<'info, T>,
    token_vault: &Account<'info, TokenAccount>,
    token_account: &Account<'info, TokenAccount>,
    token_program: &Program<'info, Token>,
    amount: u64,
    seeds: &[&[u8]],
) -> Result<()> {
    transfer(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            Transfer {
                from: token_vault.to_account_info(),
                to: token_account.to_account_info(),
                authority: authority.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )
}
