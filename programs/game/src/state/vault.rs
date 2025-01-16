use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

#[account]
#[derive(Debug, Default, InitSpace)]
pub struct Vault {
    /// Mint information for token A
    pub token_mint: Pubkey,

    /// Token A
    pub token_vault: Pubkey,

    /// Amount of token B
    pub token_amount: u64,
}

impl Vault {
    pub fn initialize(
        &mut self,
        token_mint: Pubkey,
        token_vault: Pubkey,
        token_amount: u64,
    ) -> Result<()> {
        *self = Vault {
            token_mint,
            token_vault,
            token_amount,
            ..Default::default()
        };

        Ok(())
    }

    pub fn deposit(&mut self, token_amount: u64) -> Result<()> {
        self.token_amount = self.token_amount.safe_sub(token_amount)?;

        Ok(())
    }
}
