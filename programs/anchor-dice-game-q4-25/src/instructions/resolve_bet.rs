use crate::{errors::DiceError, Bet};
use anchor_instruction_sysvar::{Ed25519InstructionSignatures};
use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};
use solana_program::{
    ed25519_program, hash::hash, sysvar::instructions::load_instruction_at_checked
};

#[constant]
const HOUSE_FEE: u64 = 150; // basis point -> 1.5%

#[derive(Accounts)]
pub struct ResolveBet<'info> {
    #[account(mut)]
    pub house: Signer<'info>,
    ///CHECK: this is safe
    #[account(mut)]
    pub player: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"vault", house.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,
       #[account(
        mut,
        has_one = player,
        close = player,
        seeds = [b"bet", vault.key().as_ref(), bet.seed.to_le_bytes().as_ref()],
        bump = bet.bump
    )]
    pub bet: Account<'info, Bet>,
    ///CHECK: this is safe
    #[account(
        address = solana_program::sysvar::instructions::ID
    )]
    pub instruction_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>
}


impl<'info> ResolveBet<'info> {

    pub fn verify_ed25519_signature(&self, sig: &[u8]) -> Result<()> {
       
        let ix = load_instruction_at_checked(0, &self.instruction_sysvar.to_account_info()).map_err(|_| DiceError::Ed25519Program)?;

        //check if instruction's program Id is from ed25519 program
        require_eq!(ix.program_id, ed25519_program::ID, DiceError::Ed25519Program);

        //ensure no accounts in instruction
        require_eq!(ix.accounts.len(), 0, DiceError::Ed25519Accounts);


        let signatures = Ed25519InstructionSignatures::unpack(&ix.data).map_err(|_| DiceError::Ed25519Signature)?.0;

        //check numbers of signatures
        require_eq!(signatures.len(), 1, DiceError::Ed25519Signature);

        let signature = &signatures[0];

        // ensure the signature is verifiable
        require!(signature.is_verifiable, DiceError::Ed25519Header);

        require_keys_eq!(signature.public_key.ok_or(DiceError::Ed25519Pubkey)?, self.house.key());

        require!(&signature.signature.ok_or(DiceError::Ed25519Signature)?.eq(sig), DiceError::Ed25519Signature);

        require!(&signature.message.as_ref().ok_or(DiceError::Ed25519Message)?.eq(&self.bet.to_slice()), DiceError::Ed25519Message);


        Ok(())
    }

    pub fn resolve_bet(&self, bumps: &ResolveBetBumps, sig: &[u8]) -> Result<()> {
        let hash = hash(sig).to_bytes();
        let mut hash_16: [u8; 16] = [0; 16];
        hash_16.copy_from_slice(&hash[0..16]);
        let lower: u128 = u128::from_le_bytes(hash_16);
        
        hash_16.copy_from_slice(&hash[16..32]);
        let upper = u128::from_le_bytes(hash_16);

        let roll = lower.wrapping_add(upper).wrapping_rem(100) as u8 + 1;
        

        if self.bet.roll > roll {
            let payout = (self.bet.amount as u128)
                .checked_mul(10000 - HOUSE_FEE as u128).ok_or(DiceError::Overflow)?
                // .checked_div(self.bet.roll as u128 -1).ok_or(DiceError::Overflow)?
                .checked_div(100).ok_or(DiceError::Overflow)?;

            let signer_seeds : &[&[&[u8]]] = &[
                &[
                    b"vault",
                    &self.house.key().to_bytes(),
                    &[bumps.vault]
                ]
            ];

            let accounts = Transfer {
                from: self.vault.to_account_info(),
                to: self.player.to_account_info()
            };

            let cpi_ctx = CpiContext::new_with_signer(
                self.system_program.to_account_info(), 
                accounts,
                signer_seeds
            );
            transfer(cpi_ctx, payout as u64)?;
        }
        Ok(())
    }

}
