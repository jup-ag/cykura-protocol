pub mod context;
pub mod error;
pub mod libraries;
pub mod states;
use crate::context::*;
use crate::error::ErrorCode;
use crate::states::factory::OwnerChangedEvent;
use crate::states::fee::FeeAmountEnabledEvent;
use crate::states::pool::*;
use crate::states::position::*;
use anchor_lang::prelude::*;

declare_id!("37kn8WUzihQoAnhYxueA2BnqCA7VRnrVvYoHy1hQ6Veu");

#[program]
pub mod cyclos_protocol_v2 {
    use super::*;
    use crate::libraries::tick_math::get_tick_at_sqrt_price;
    use anchor_lang::solana_program::system_program;
    use anchor_spl::{associated_token, token};

    // ---------------------------------------------------------------------
    // 1. Factory instructions

    pub fn initialize(ctx: Context<Initialize>, bump: u8) -> ProgramResult {
        ctx.accounts.factory_state.bump = bump;
        ctx.accounts.factory_state.owner = ctx.accounts.owner.key();

        emit!(OwnerChangedEvent {
            old_owner: system_program::ID,
            new_owner: ctx.accounts.owner.key(),
        });

        Ok(())
    }

    pub fn enable_fee_amount(
        ctx: Context<EnableFeeAmount>,
        fee: u32,
        tick_spacing: u16,
        fee_bump: u8,
    ) -> ProgramResult {
        if fee > 1_000_000 {
            // 100% fee
            return Err(ErrorCode::FeeLimit.into());
        }

        // TODO find why uni uses i24 and max 16384 for tickSpacing
        if tick_spacing > 16384 {
            return Err(ErrorCode::TickSpacingLimit.into());
        }

        emit!(FeeAmountEnabledEvent { fee, tick_spacing });

        ctx.accounts.fee_state.bump = fee_bump;
        ctx.accounts.fee_state.fee = fee;
        ctx.accounts.fee_state.tick_spacing = tick_spacing;

        Ok(())
    }

    pub fn set_owner(ctx: Context<SetOwner>) -> ProgramResult {
        ctx.accounts.factory_state.owner = ctx.accounts.new_owner.key();

        emit!(OwnerChangedEvent {
            old_owner: ctx.accounts.owner.key(),
            new_owner: ctx.accounts.new_owner.key(),
        });

        Ok(())
    }

    // ---------------------------------------------------------------------
    // 2. Pool instructions

    /// Create pool and initialize with desired price
    /// Create pool PDA for [token0, token1, fee] where tokenA > tokenB,
    /// then set sqrt_price
    ///
    /// Single function in place of Factory.createPool(), PoolDeployer.deploy()
    /// Pool.initialize() and pool.Constructor()
    pub fn create_pool(
        ctx: Context<CreatePool>,
        pool_state_bump: u8,
        fee: u32,
        sqrt_price: f64,
    ) -> ProgramResult {
        // let state = PoolState {
        //     bump: pool_state_bump,
        //     token_0: (*ctx.accounts.token_0).key(),
        //     token_1: (*ctx.accounts.token_1).key(),
        //     fee,
        //     tick_spacing: (*ctx.accounts.fee_state).tick_spacing,
        //     liquidity: 0,
        //     sqrt_price,
        //     tick: get_tick_at_sqrt_price(sqrt_price),
        //     fee_growth_global_0: 0.0,
        //     fee_growth_global_1: 0.0,
        //     fee_protocol: 0, // Leftmost 4 bits: fee_token_0, rightmost 4 bits: fee_token_1
        //     protocol_fees_token_0: 0,
        //     protocol_fees_token_1: 0,
        //     unlocked: true,
        // };

        // ctx.accounts.pool_state.clone()
        // *ctx.accounts.pool_state = state as Account;

        let tick = get_tick_at_sqrt_price(sqrt_price);

        // Set pool state
        ctx.accounts.pool_state.bump = pool_state_bump;
        ctx.accounts.pool_state.token_0 = (*ctx.accounts.token_0).key();
        ctx.accounts.pool_state.token_1 = (*ctx.accounts.token_1).key();
        ctx.accounts.pool_state.fee = fee;
        ctx.accounts.pool_state.tick_spacing = (*ctx.accounts.fee_state).tick_spacing;
        ctx.accounts.pool_state.sqrt_price = sqrt_price;
        ctx.accounts.pool_state.tick = tick;
        ctx.accounts.pool_state.unlocked = true;
        // protocol fee initially set as 0

        // create associated token accounts for pool, which act as pool vaults
        if !(*ctx.accounts.vault_0).to_account_info().executable {
            let create_vault_0_ctx = CpiContext::new(
                ctx.accounts.associated_token_program.to_account_info(),
                associated_token::Create {
                    payer: ctx.accounts.pool_creator.to_account_info(),
                    associated_token: ctx.accounts.vault_0.to_account_info(),
                    authority: ctx.accounts.pool_state.to_account_info(),
                    mint: ctx.accounts.token_0.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            );
            associated_token::create(create_vault_0_ctx)?;
        }
        if !(*ctx.accounts.vault_1).to_account_info().executable {
            let create_vault_1_ctx = CpiContext::new(
                ctx.accounts.associated_token_program.to_account_info(),
                associated_token::Create {
                    payer: ctx.accounts.pool_creator.to_account_info(),
                    associated_token: ctx.accounts.vault_1.to_account_info(),
                    authority: ctx.accounts.pool_state.to_account_info(),
                    mint: ctx.accounts.token_1.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            );
            associated_token::create(create_vault_1_ctx)?;
        }

        emit!(InitPoolEvent {
            pool_state: (*ctx.accounts.pool_state).key(),
            token_0: (*ctx.accounts.token_0).key(),
            token_1: (*ctx.accounts.token_1).key(),
            fee,
            sqrt_price,
            tick,
        });

        Ok(())
    }

    // ---------------------------------------------------------------------
    // 3. Position instructions

    /// Add liquidity for the given position
    /// Only callable by a smart contract which implements mintCallback()
    /// Periphery.LiquidityManagement.addLiquidity() -> Core.mint()
    ///     -> Periphery.LiquidityManagement.uniswapV3MintCallback()
    /// Due tokens must be paid in uniswapV3MintCallback()
    /// TODO study periphery and see what data field does
    pub fn mint(
        ctx: Context<Todo>,
        tick_lower: i32,
        tick_upper: i32,
        amount: u32,
    ) -> ProgramResult {
        // TODO convert tick_lower and tick_upper to i24
        todo!()
    }

    /// Collect tokens owed to a position
    /// Owed = fees + burned tokens
    /// 'Burned' tokens are tokens made inactive in a position, but are yet to be withdrawn
    /// Look at burn()
    /// Read position details (tick_upper, tick_lower) from the Position PDA
    pub fn collect(
        ctx: Context<Todo>,
        amount_0_requested: u64,
        amount_1_requested: u64,
    ) -> ProgramResult {
        todo!()
    }

    /// Reduce liquidity in a position by given amount
    /// 'Burned' tokens are tokens made inactive in a position,
    /// but are not yet withdrawn
    pub fn burn(ctx: Context<Todo>, amount: u32) -> ProgramResult {
        todo!()
    }

    // ---------------------------------------------------------------------
    // 4. Swap instructions

    /// Perform swap
    ///
    /// Only callable by smart contract which implements uniswapV3SwapCallback()
    ///
    /// Flow
    /// 1. Periphery.SwapRouter.exactInputInternal()/exactOutputInternal(): stateless routing
    /// 2. Core.UniswapV3Pool.swap(): change state
    /// 3. Periphery.SwapRouter.uniswapV3SwapCallback(): transfer tokens from user to pool
    ///
    /// @param zero_for_one Swap token0 -> token1 if true, else token1 -> token0
    /// @param amount_specified Δtoken0 or Δtoken1 to be added/removed to pool.
    /// Exact input swap if positive, else exact output swap
    /// @param sqrt_price_limit Limit price √P for slippage
    pub fn swap(
        ctx: Context<Todo>,
        zero_for_one: bool,
        amount_specified: i64,
        sqrt_price_limit: f64,
    ) -> ProgramResult {
        todo!()
    }

    /// Component function for flash swaps
    ///
    /// Donate given liquidity to in-range positions then make callback
    /// Only callable by a smart contract which implements uniswapV3FlashCallback(),
    /// where profitability check can be performed
    ///
    /// Flash swaps is an advanced feature for developers, not directly available for UI based traders.
    /// Periphery does not provide an implementation, but a sample is provided
    /// Ref- https://github.com/Uniswap/v3-periphery/blob/main/contracts/examples/PairFlash.sol
    ///
    ///
    /// Flow
    /// 1. FlashDapp.initFlash()
    /// 2. Core.flash()
    /// 3. FlashDapp.uniswapV3FlashCallback()
    ///
    /// @param amount_0 Amount of token 0 to donate
    /// @param amount_1 Amount of token 1 to donate
    pub fn flash(ctx: Context<Todo>, amount_0: u64, amount_1: u64) -> ProgramResult {
        todo!()
    }

    // ---------------------------------------------------------------------
    // 5. Pool owner instructions

    /// Update protocol fees for a pool
    /// Protocol fee can be 0 or 1/N where 4 <= N <= 10 (fits in 4 bits)
    /// Both tokens in the pool can have different protocol fees
    /// Compress as a single u8, where fee_protocol_1 are leftmost bits and fee_protocol_0 are rightmost
    pub fn set_fee_protocol(
        ctx: Context<SetFeeProtocol>,
        fee_protocol_0: u8,
        fee_protocol_1: u8,
    ) -> ProgramResult {
        if !ctx.accounts.pool_state.unlocked {
            return Err(ErrorCode::Locked.into());
        }
        ctx.accounts.pool_state.unlocked = false;

        if (fee_protocol_0 == 0 || (fee_protocol_0 >= 4 && fee_protocol_0 <= 10))
            && (fee_protocol_1 == 0 || (fee_protocol_1 >= 4 && fee_protocol_1 <= 10))
        {
            msg!("Error: Protocol fee should be 0 or 1/N where 4 <= N <= 10 ")
        }

        let fee_protocol_old = ctx.accounts.pool_state.fee_protocol;
        // 8 bits = [4 bits of fee_protocol_1][4 bits of fee_protocol_0]
        ctx.accounts.pool_state.fee_protocol = (fee_protocol_1 << 4) + fee_protocol_0;

        emit!(SetFeeProtocolEvent {
            pool_state: ctx.accounts.pool_state.key(),
            fee_protocol_0_old: fee_protocol_old % 16,
            fee_protocol_1_old: fee_protocol_old >> 4,
            fee_protocol_0,
            fee_protocol_1,
        });

        ctx.accounts.pool_state.unlocked = true;
        Ok(())
    }

    /// Collect protocol fees
    /// Amounts can be 0 to collect fees only in the other token
    pub fn collect_protocol(
        ctx: Context<CollectProtocol>,
        amount_0_requested: u64,
        amount_1_requested: u64,
    ) -> ProgramResult {
        if !ctx.accounts.pool_state.unlocked {
            return Err(ErrorCode::Locked.into());
        }
        ctx.accounts.pool_state.unlocked = false;
        let pool_state = &mut *ctx.accounts.pool_state;

        // Amounts to be transferred to owner = MIN (requested, accrued)
        // Cannot transfer out more than accrued
        let mut amount_0 = if amount_0_requested > pool_state.protocol_fees_token_0 {
            pool_state.protocol_fees_token_0
        } else {
            amount_0_requested
        };
        let mut amount_1 = if amount_1_requested > pool_state.protocol_fees_token_1 {
            pool_state.protocol_fees_token_1
        } else {
            amount_1_requested
        };

        let token_0 = pool_state.token_0.clone();
        let token_1 = pool_state.token_1.clone();

        let seeds = &[
            &token_0.to_bytes() as &[u8],
            &token_1.to_bytes() as &[u8],
            &pool_state.fee.to_be_bytes() as &[u8],
            &[pool_state.bump],
        ];
        let signer_seeds = &[&seeds[..]];

        if amount_0 > 0 {
            // Note- Uniswap leaves out 1 fee unit in state so
            // register is not cleared. This saves gas.
            // If there are 100 unclaimed units, maxiumum 99 can be sent
            // Retained for API compatibility
            if amount_0 == pool_state.protocol_fees_token_0 {
                amount_0 = amount_0.checked_sub(1).unwrap();
            }

            pool_state.protocol_fees_token_0 = pool_state
                .protocol_fees_token_0
                .checked_sub(amount_0)
                .unwrap();

            // Transfer
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info().clone(),
                    token::Transfer {
                        from: ctx.accounts.vault_0.to_account_info().clone(),
                        to: ctx.accounts.owner_wallet_0.to_account_info().clone(),
                        authority: pool_state.to_account_info().clone(),
                    },
                    signer_seeds,
                ),
                amount_0,
            )?;
        }
        if amount_1 > 0 {
            // Note- Uniswap leaves out 1 fee unit in state so
            // register is not cleared. This saves gas.
            // If there are 100 unclaimed units, maxiumum 99 can be sent
            if amount_1 == pool_state.protocol_fees_token_1 {
                amount_1 = amount_1.checked_sub(1).unwrap();
            }
            
            pool_state.protocol_fees_token_1 = pool_state
                .protocol_fees_token_1
                .checked_sub(amount_1)
                .unwrap();

            // Transfer
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info().clone(),
                    token::Transfer {
                        from: ctx.accounts.vault_1.to_account_info().clone(),
                        to: ctx.accounts.owner_wallet_1.to_account_info().clone(),
                        authority: pool_state.to_account_info().clone(),
                    },
                    signer_seeds,
                ),
                amount_1,
            )?;
        }

        emit!(CollectProtocolEvent {
            pool_state: pool_state.key(),
            amount_0,
            amount_1,
        });
        ctx.accounts.pool_state.unlocked = true;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Todo {}

/// Update position with given liquidity_delta
/// Skipped TWAP calculation for now.
/// Position liquidity and flipped state in bitmap is updated
/// From Pools._update_position()
pub fn update_position(position: PositionState, pool: PoolState, liquidity_delta: u32, tick: i32) {
    // update the ticks if liquidity present
    if liquidity_delta != 0 {
        // Skip TWAP things for now.
    }
    todo!();
}

/// Update position with new liquidity, and find Δtoken0 and Δtoken1 required
/// to produce this liquidity_delta
/// mint() -> modify_position() -> update_position() -> update()
///
/// TODO check what noDelegateCall does
pub fn modify_position(
    position: PositionState,
    pool: PoolState,
    liquidity_delta: u32,
) -> (i64, i64) {
    todo!()
}
