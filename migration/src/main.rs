use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};
use anchor_spl::token::{self, Token, Transfer};

declare_id!("YOUR_PROGRAM_ID_HERE");

// BACH Token on Devnet: DENNuKzCcrLhEtxZ8tm7nSeef8qvKgGGrdxX6euNkNS7
// New Token on Devnet (Token-2022): [TO BE DEPLOYED]

#[program]
pub mod bach_token_swap {
    use super::*;

    /// Initialize the swap program with the vault authority
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let swap_state = &mut ctx.accounts.swap_state;
        swap_state.authority = ctx.accounts.authority.key();
        swap_state.old_token_mint = ctx.accounts.old_token_mint.key();
        swap_state.new_token_mint = ctx.accounts.new_token_mint.key();
        swap_state.old_token_vault = ctx.accounts.old_token_vault.key();
        swap_state.new_token_vault = ctx.accounts.new_token_vault.key();
        swap_state.bump = ctx.bumps.swap_state;
        swap_state.swap_ratio_numerator = 1;
        swap_state.swap_ratio_denominator = 1;
        swap_state.total_swapped = 0;
        
        msg!("Swap program initialized!");
        msg!("Old token mint: {}", swap_state.old_token_mint);
        msg!("New token mint: {}", swap_state.new_token_mint);
        
        Ok(())
    }

    /// Swap old BACH tokens for new Token-2022 BACH tokens
    pub fn swap_tokens(ctx: Context<SwapTokens>, amount: u64) -> Result<()> {
        require!(amount > 0, SwapError::InvalidAmount);
        
        let swap_state = &mut ctx.accounts.swap_state;
        
        // Calculate the amount of new tokens to give (1:1 ratio by default)
        let new_token_amount = amount
            .checked_mul(swap_state.swap_ratio_numerator)
            .ok_or(SwapError::Overflow)?
            .checked_div(swap_state.swap_ratio_denominator)
            .ok_or(SwapError::Overflow)?;

        require!(new_token_amount > 0, SwapError::InvalidAmount);

        // Transfer old BACH tokens from user to vault
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_old_token_account.to_account_info(),
            to: ctx.accounts.old_token_vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Transfer new Token-2022 BACH tokens from vault to user
        let seeds = &[
            b"swap_state",
            &[swap_state.bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.new_token_vault.to_account_info(),
            mint: ctx.accounts.new_token_mint.to_account_info(),
            to: ctx.accounts.user_new_token_account.to_account_info(),
            authority: ctx.accounts.swap_state.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_2022_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        
        token_interface::transfer_checked(
            cpi_ctx,
            new_token_amount,
            ctx.accounts.new_token_mint.decimals,
        )?;

        // Update total swapped
        swap_state.total_swapped = swap_state.total_swapped
            .checked_add(amount)
            .ok_or(SwapError::Overflow)?;

        emit!(SwapEvent {
            user: ctx.accounts.user.key(),
            old_token_amount: amount,
            new_token_amount,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("Swap successful! Old: {} -> New: {}", amount, new_token_amount);

        Ok(())
    }

    /// Update swap ratio (admin only)
    pub fn update_swap_ratio(
        ctx: Context<UpdateSwapRatio>,
        numerator: u64,
        denominator: u64,
    ) -> Result<()> {
        require!(denominator > 0, SwapError::InvalidRatio);
        require!(numerator > 0, SwapError::InvalidRatio);
        
        let swap_state = &mut ctx.accounts.swap_state;
        swap_state.swap_ratio_numerator = numerator;
        swap_state.swap_ratio_denominator = denominator;
        
        msg!("Swap ratio updated to {}:{}", numerator, denominator);
        
        Ok(())
    }

    /// Withdraw tokens from vault (admin only, emergency function)
    pub fn withdraw_tokens(
        ctx: Context<WithdrawTokens>,
        amount: u64,
        withdraw_old: bool,
    ) -> Result<()> {
        let swap_state = &ctx.accounts.swap_state;
        let seeds = &[
            b"swap_state",
            &[swap_state.bump],
        ];
        let signer = &[&seeds[..]];

        if withdraw_old {
            // Withdraw old tokens
            let cpi_accounts = Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.authority_token_account.to_account_info(),
                authority: ctx.accounts.swap_state.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
            token::transfer(cpi_ctx, amount)?;
        } else {
            // Withdraw new tokens (Token-2022)
            let cpi_accounts = TransferChecked {
                from: ctx.accounts.vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.authority_token_account.to_account_info(),
                authority: ctx.accounts.swap_state.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_2022_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
            
            token_interface::transfer_checked(
                cpi_ctx,
                amount,
                ctx.accounts.mint.decimals,
            )?;
        }

        msg!("Withdrawal successful: {} tokens", amount);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + SwapState::INIT_SPACE,
        seeds = [b"swap_state"],
        bump
    )]
    pub swap_state: Account<'info, SwapState>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// Old BACH token mint
    pub old_token_mint: Account<'info, token::Mint>,
    
    /// New BACH token mint (Token-2022)
    pub new_token_mint: InterfaceAccount<'info, Mint>,
    
    /// Vault to hold old BACH tokens
    #[account(
        mut,
        token::mint = old_token_mint,
        token::authority = authority,
    )]
    pub old_token_vault: Account<'info, token::TokenAccount>,
    
    /// Vault to hold new BACH tokens (Token-2022)
    /// Authority should be the swap_state PDA
    #[account(
        mut,
        token::mint = new_token_mint,
    )]
    pub new_token_vault: InterfaceAccount<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SwapTokens<'info> {
    #[account(
        mut,
        seeds = [b"swap_state"],
        bump = swap_state.bump,
    )]
    pub swap_state: Account<'info, SwapState>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    /// User's old BACH token account
    #[account(
        mut,
        token::mint = swap_state.old_token_mint,
        token::authority = user,
    )]
    pub user_old_token_account: Account<'info, token::TokenAccount>,
    
    /// User's new BACH token account (Token-2022)
    #[account(
        mut,
        token::mint = swap_state.new_token_mint,
    )]
    pub user_new_token_account: InterfaceAccount<'info, TokenAccount>,
    
    /// Program's old token vault
    #[account(
        mut,
        address = swap_state.old_token_vault,
    )]
    pub old_token_vault: Account<'info, token::TokenAccount>,
    
    /// Program's new token vault
    #[account(
        mut,
        address = swap_state.new_token_vault,
    )]
    pub new_token_vault: InterfaceAccount<'info, TokenAccount>,
    
    #[account(address = swap_state.old_token_mint)]
    pub old_token_mint: Account<'info, token::Mint>,
    
    #[account(address = swap_state.new_token_mint)]
    pub new_token_mint: InterfaceAccount<'info, Mint>,
    
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct UpdateSwapRatio<'info> {
    #[account(
        mut,
        seeds = [b"swap_state"],
        bump = swap_state.bump,
        has_one = authority,
    )]
    pub swap_state: Account<'info, SwapState>,
    
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawTokens<'info> {
    #[account(
        seeds = [b"swap_state"],
        bump = swap_state.bump,
        has_one = authority,
    )]
    pub swap_state: Account<'info, SwapState>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(mut)]
    pub vault: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority_token_account: AccountInfo<'info>,
    
    pub mint: InterfaceAccount<'info, Mint>,
    
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Interface<'info, TokenInterface>,
}

#[account]
#[derive(InitSpace)]
pub struct SwapState {
    pub authority: Pubkey,
    pub old_token_mint: Pubkey,
    pub new_token_mint: Pubkey,
    pub old_token_vault: Pubkey,
    pub new_token_vault: Pubkey,
    pub bump: u8,
    pub swap_ratio_numerator: u64,
    pub swap_ratio_denominator: u64,
    pub total_swapped: u64,
}

#[event]
pub struct SwapEvent {
    pub user: Pubkey,
    pub old_token_amount: u64,
    pub new_token_amount: u64,
    pub timestamp: i64,
}

#[error_code]
pub enum SwapError {
    #[msg("Invalid swap ratio")]
    InvalidRatio,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Arithmetic overflow")]
    Overflow,
}