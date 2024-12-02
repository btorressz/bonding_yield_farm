# bonding_yield_farm
# Bonding Curve-Powered Yield Farming on Solana

This project implements a bonding curve-powered yield farming system using the Solana blockchain with the Anchor framework. It allows users to stake tokens, earn rewards dynamically, and manage liquidity with robust security features. This project was mainly developed in https://beta.solpg.io/ 

## Features

### Dynamic Yield Farming Rewards
- Rewards are distributed based on a bonding curve.
- Early liquidity providers earn higher returns.

### Lockup Periods
- Users can set optional lockup periods for boosted rewards.
- Stake is locked for the specified duration before withdrawal.

### Anti-Whale Mechanisms
- Individual user deposit limits.
- Total pool liquidity cap.

### Emergency Pause
- Admins can pause the pool to restrict staking and withdrawals.

### Reward Tracking
- Tracks total rewards distributed for transparency.

### Comprehensive Events
- Emits events for key actions like staking, withdrawals, and admin actions.
