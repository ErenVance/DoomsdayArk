use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

#[account]
#[derive(Debug, Default, InitSpace)]
/// The `Voucher` account represents a tokenized representation of underlying staked or pooled assets in the system.
/// Vouchers can be minted and burned to track proportional ownership or claims on the pool.
/// This structure keeps track of the voucher mint, associated vault, and both the currently minted amount and the total supply.
pub struct Voucher {
    /// The public key of the voucher's mint account.
    pub voucher_mint: Pubkey,

    /// The public key of the voucher vault where the vouchers (or their underlying value) are stored.
    pub voucher_vault: Pubkey,

    /// The amount of vouchers that have been minted but not necessarily in circulation yet.
    /// This can be used to track recent issuance before distribution.
    pub minted_amount: u64,

    /// The total supply of vouchers that have been issued and not burned.
    /// Represents the cumulative amount of vouchers ever minted minus those burned.
    pub total_supply: u64,
}

impl Voucher {
    /// Initializes the voucher configuration with the provided mint and vault public keys.
    /// Sets the voucher to a default state with no vouchers minted.
    ///
    /// # Arguments
    /// - `voucher_mint`: The public key of the token mint used for issuing vouchers.
    /// - `voucher_vault`: The public key of the vault account holding voucher tokens or related assets.
    ///
    /// # Returns
    /// `Ok(())` if successfully initialized.
    pub fn initialize(&mut self, voucher_mint: Pubkey, voucher_vault: Pubkey) -> Result<()> {
        *self = Voucher {
            voucher_mint,
            voucher_vault,
            ..Default::default()
        };

        Ok(())
    }

    /// Mints a specified `amount` of vouchers, increasing both the `minted_amount` and `total_supply`.
    /// This function should be called when new vouchers are issued to users or internal accounts.
    ///
    /// # Arguments
    /// - `amount`: The number of vouchers to be minted.
    ///
    /// # Returns
    /// `Ok(())` if the minting operation is successful, otherwise an error indicating arithmetic overflow.
    pub fn mint(&mut self, amount: u64) -> Result<()> {
        self.minted_amount = self.minted_amount.safe_add(amount)?;
        self.total_supply = self.total_supply.safe_add(amount)?;
        Ok(())
    }

    /// Burns a specified `amount` of vouchers, decreasing the `total_supply`.
    /// This function should be called when vouchers are redeemed, destroyed, or otherwise removed from circulation.
    ///
    /// # Arguments
    /// - `amount`: The number of vouchers to be burned.
    ///
    /// # Returns
    /// `Ok(())` if the burning operation is successful, otherwise an error indicating insufficient supply or arithmetic issue.
    pub fn burn(&mut self, amount: u64) -> Result<()> {
        self.total_supply = self.total_supply.safe_sub(amount)?;
        Ok(())
    }
}
