#![cfg_attr(not(feature = "std"), no_std)]
#![allow(non_snake_case)]

use ink_lang as ink;

const PRECISION: u128 = 1_000_000;

#[ink::contract]
mod amm {
    use ink_storage::collections::HashMap;

    #[derive(Default)]
    #[ink(storage)]
    pub struct Amm {
        totalShares: Balance,
        totalToken1: Balance,
        totalToken2: Balance,
        K: Balance,
        shares: HashMap<AccountId, Balance>,
        token1Balance: HashMap<AccountId, Balance>,
        token2Balance: HashMap<AccountId, Balance>,
    }

    impl Amm {
        #[ink(constructor)]
        pub fn new() -> Self {
            Default::default()
        }

        #[ink(message)]
        pub fn faucet(&mut self, _amountToken1: Balance, _amountToken2: Balance) {
            let caller = self.env().caller();
            let token1 = *self.token1Balance.get(&caller).unwrap_or(&0);
            let token2 = *self.token2Balance.get(&caller).unwrap_or(&0);

            self.token1Balance.insert(caller, token1 + _amountToken1);
            self.token2Balance.insert(caller, token2 + _amountToken2);
        }

        #[ink(message)]
        pub fn getMyHoldings(&self) -> (Balance, Balance, Balance) {
            let caller = self.env().caller();
            let token1 = *self.token1Balance.get(&caller).unwrap_or(&0);
            let token2 = *self.token2Balance.get(&caller).unwrap_or(&0);
            let myShares = *self.shares.get(&caller).unwrap_or(&0);
            (token1, token2, myShares)
        }

        #[ink(message)]
        pub fn getPoolDetails(&self) -> (Balance, Balance, Balance) {
            (self.totalToken1, self.totalToken2, self.totalShares)
        }

        fn activePool(&self) -> Result<(), &'static str> {
            match self.K {
                0 => Err("Zero Liquidity"),
                _ => Ok(()),
            }
        }

        #[ink(message)]
        pub fn getEquivalentToken1Estimate(
            &self,
            _amountToken2: Balance,
        ) -> Result<Balance, &'static str> {
            self.activePool()?;
            Ok(self.totalToken1 * _amountToken2 / self.totalToken2)
        }

        #[ink(message)]
        pub fn getEquivalentToken2Estimate(
            &self,
            _amountToken1: Balance,
        ) -> Result<Balance, &'static str> {
            self.activePool()?;
            Ok(self.totalToken2 * _amountToken1 / self.totalToken1)
        }

        fn validAmountCheck(
            &self,
            _balance: &HashMap<AccountId, Balance>,
            _qty: Balance,
        ) -> Result<(), &'static str> {
            let caller = self.env().caller();
            let my_balance = *_balance.get(&caller).unwrap_or(&0);

            match _qty {
                0 => Err("Amount cannot be zero!"),
                _ if _qty > my_balance => Err("Insufficient amount"),
                _ => Ok(()),
            }
        }

        #[ink(message)]
        pub fn provide(
            &mut self,
            _amountToken1: Balance,
            _amountToken2: Balance,
        ) -> Result<Balance, &'static str> {
            self.validAmountCheck(&self.token1Balance, _amountToken1)?;
            self.validAmountCheck(&self.token2Balance, _amountToken2)?;

            let share;
            if self.totalShares == 0 {
                share = 100 * super::PRECISION;
            } else {
                let share1 = self.totalShares * _amountToken1 / self.totalToken1;
                let share2 = self.totalShares * _amountToken2 / self.totalToken2;

                if share1 != share2 {
                    return Err("Equivalent value of tokens not provided...");
                }
                share = share1;
            }

            if share == 0 {
                return Err("Asset value less than threshold for contribution!");
            }

            let caller = self.env().caller();
            let token1 = *self.token1Balance.get(&caller).unwrap();
            let token2 = *self.token2Balance.get(&caller).unwrap();
            self.token1Balance.insert(caller, token1 - _amountToken1);
            self.token2Balance.insert(caller, token2 - _amountToken2);
            self.totalToken1 += _amountToken1;
            self.totalToken2 += _amountToken2;
            self.K = self.totalToken1 * self.totalToken2;

            self.totalShares += share;
            self.shares
                .entry(caller)
                .and_modify(|val| *val += share)
                .or_insert(share);

            Ok(share)
        }

        #[ink(message)]
        pub fn getWithdrawEstimate(
            &self,
            _share: Balance,
        ) -> Result<(Balance, Balance), &'static str> {
            self.activePool()?;
            if _share > self.totalShares {
                return Err("Share should be less than totalShare");
            }

            let amountToken1 = _share * self.totalToken1 / self.totalShares;
            let amountToken2 = _share * self.totalToken2 / self.totalShares;
            Ok((amountToken1, amountToken2))
        }

        #[ink(message)]
        pub fn withdraw(&mut self, _share: Balance) -> Result<(Balance, Balance), &'static str> {
            let caller = self.env().caller();
            self.validAmountCheck(&self.shares, _share)?;

            let (amountToken1, amountToken2) = self.getWithdrawEstimate(_share)?;
            self.shares.entry(caller).and_modify(|val| *val -= _share);
            self.totalShares -= _share;

            self.totalToken1 -= amountToken1;
            self.totalToken2 -= amountToken2;
            self.K = self.totalToken1 * self.totalToken2;
            self.token1Balance
                .entry(caller)
                .and_modify(|val| *val += amountToken1);
            self.token2Balance
                .entry(caller)
                .and_modify(|val| *val += amountToken2);

            Ok((amountToken1, amountToken2))
        }

        #[ink(message)]
        pub fn getSwapToken1Estimate(
            &self,
            _amountToken1: Balance,
        ) -> Result<Balance, &'static str> {
            self.activePool()?;
            let token1After = self.totalToken1 + _amountToken1;
            let token2After = self.K / token1After;
            let mut amountToken2 = self.totalToken2 - token2After;
            if amountToken2 == self.totalToken2 {
                amountToken2 -= 1;
            }
            Ok(amountToken2)
        }

        #[ink(message)]
        pub fn getSwapToken1EstimateGivenToken2(
            &self,
            _amountToken2: Balance,
        ) -> Result<Balance, &'static str> {
            self.activePool()?;
            if _amountToken2 >= self.totalToken2 {
                return Err("Insufficient pool balance");
            }

            let token2After = self.totalToken2 - _amountToken2;
            let token1After = self.K / token2After;
            let amountToken1 = token1After - self.totalToken1;
            Ok(amountToken1)
        }

        #[ink(message)]
        pub fn swapToken1(&mut self, _amountToken1: Balance) -> Result<Balance, &'static str> {
            let caller = self.env().caller();
            self.validAmountCheck(&self.token1Balance, _amountToken1)?;
            let amountToken2 = self.getSwapToken1Estimate(_amountToken1)?;
            self.token1Balance
                .entry(caller)
                .and_modify(|val| *val -= _amountToken1);
            self.totalToken1 += _amountToken1;
            self.totalToken2 -= amountToken2;
            self.token2Balance
                .entry(caller)
                .and_modify(|val| *val += amountToken2);
            Ok(amountToken2)
        }

        #[ink(message)]
        pub fn getSwapToken2Estimate(
            &self,
            _amountToken2: Balance,
        ) -> Result<Balance, &'static str> {
            self.activePool()?;
            let token2After = self.totalToken2 + _amountToken2;
            let token1After = self.K / token2After;
            let mut amountToken1 = self.totalToken1 - token1After;
            if amountToken1 == self.totalToken1 {
                amountToken1 -= 1;
            }
            Ok(amountToken1)
        }

        #[ink(message)]
        pub fn getSwapToken2EstimateGivenToken1(
            &self,
            _amountToken1: Balance,
        ) -> Result<Balance, &'static str> {
            self.activePool()?;
            if _amountToken1 >= self.totalToken1 {
                return Err("Insufficient pool balance");
            }

            let token1After = self.totalToken1 - _amountToken1;
            let token2After = self.K / token1After;
            let amountToken2 = token2After - self.totalToken2;
            Ok(amountToken2)
        }

        #[ink(message)]
        pub fn swapToken2(&mut self, _amountToken2: Balance) -> Result<Balance, &'static str> {
            let caller = self.env().caller();
            self.validAmountCheck(&self.token2Balance, _amountToken2)?;
            let amountToken1 = self.getSwapToken2Estimate(_amountToken2)?;
            self.token2Balance
                .entry(caller)
                .and_modify(|val| *val -= _amountToken2);
            self.totalToken2 += _amountToken2;
            self.totalToken1 -= amountToken1;
            self.token1Balance
                .entry(caller)
                .and_modify(|val| *val += amountToken1);
            Ok(amountToken1)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use ink_lang as ink;

        #[ink::test]
        fn new_works() {
            let contract = Amm::new();
            assert_eq!(contract.getMyHoldings(), (0, 0, 0));
            assert_eq!(contract.getPoolDetails(), (0, 0, 0));
        }

        #[ink::test]
        fn faucet_works() {
            let mut contract = Amm::new();
            contract.faucet(100, 200);
            assert_eq!(contract.getMyHoldings(), (100, 200, 0));
        }

        #[ink::test]
        fn zero_liquidity_test() {
            let contract = Amm::new();
            let res = contract.getEquivalentToken1Estimate(5);
            assert_eq!(res, Err("Zero Liquidity"));
        }

        #[ink::test]
        fn provide_works() {
            let mut contract = Amm::new();
            contract.faucet(100, 200);
            let share = contract.provide(10, 20).unwrap();
            assert_eq!(share, 100_000_000);
            assert_eq!(contract.getPoolDetails(), (10, 20, share));
            assert_eq!(contract.getMyHoldings(), (90, 180, share));
        }

        #[ink::test]
        fn withdraw_works() {
            let mut contract = Amm::new();
            contract.faucet(100, 200);
            let share = contract.provide(10, 20).unwrap();
            assert_eq!(contract.withdraw(share / 5).unwrap(), (2, 4));
            assert_eq!(contract.getMyHoldings(), (92, 184, 4 * share / 5));
            assert_eq!(contract.getPoolDetails(), (8, 16, 4 * share / 5));
        }

        #[ink::test]
        fn swap_works() {
            let mut contract = Amm::new();
            contract.faucet(100, 200);
            let share = contract.provide(50, 100).unwrap();
            let amountToken2 = contract.swapToken1(50).unwrap();
            assert_eq!(amountToken2, 50);
            assert_eq!(contract.getMyHoldings(), (0, 150, share));
            assert_eq!(contract.getPoolDetails(), (100, 50, share));
        }
    }
}