//! Known storage slot mappings for common DeFi protocols.
//!
//! Used by the [`Prefetcher`](super::prefetcher::Prefetcher) to proactively
//! warm cache with high-touch storage slots before simulation.

use alloy_primitives::{Address, U256};

const UNISWAP_V2_SLOTS: &[U256] = &[
    U256::from_limbs([6, 0, 0, 0]),  // reserve0 + reserve1 (packed)
    U256::from_limbs([7, 0, 0, 0]),  // blockTimestampLast
    U256::from_limbs([8, 0, 0, 0]),  // price0CumulativeLast
    U256::from_limbs([9, 0, 0, 0]),  // price1CumulativeLast
    U256::from_limbs([10, 0, 0, 0]), // kLast
];

const UNISWAP_V3_SLOTS: &[U256] = &[
    U256::from_limbs([0, 0, 0, 0]), // slot0 (sqrtPriceX96, tick, etc.)
    U256::from_limbs([1, 0, 0, 0]), // feeGrowthGlobal0X128
    U256::from_limbs([2, 0, 0, 0]), // feeGrowthGlobal1X128
    U256::from_limbs([3, 0, 0, 0]), // protocolFees
    U256::from_limbs([4, 0, 0, 0]), // liquidity
];

#[allow(dead_code)]
const ERC20_SLOTS: &[U256] = &[
    U256::from_limbs([2, 0, 0, 0]), // totalSupply (OpenZeppelin default)
];

static KNOWN_CONTRACTS: std::sync::LazyLock<
    std::collections::HashMap<Address, &'static [U256]>,
> = std::sync::LazyLock::new(|| {
    use std::collections::HashMap;
    let mut m = HashMap::new();

    // Uniswap V2 high-volume pairs
    m.insert(
        "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc".parse::<Address>().unwrap(),
        UNISWAP_V2_SLOTS as &[U256],
    );
    m.insert(
        "0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852".parse::<Address>().unwrap(),
        UNISWAP_V2_SLOTS,
    );

    // Uniswap V3 high-volume pools
    m.insert(
        "0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8".parse::<Address>().unwrap(),
        UNISWAP_V3_SLOTS,
    );
    m.insert(
        "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640".parse::<Address>().unwrap(),
        UNISWAP_V3_SLOTS,
    );
    m.insert(
        "0xCBCdF9626bC03E24f779434178A73a0B4bad62eD".parse::<Address>().unwrap(),
        UNISWAP_V3_SLOTS,
    );

    m
});

/// Returns known hot storage slots for a contract, if any.
pub fn known_slots(address: &Address) -> Option<&'static [U256]> {
    KNOWN_CONTRACTS.get(address).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_uniswap_v3_pool() {
        let usdc_weth: Address = "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640".parse().unwrap();
        let slots = known_slots(&usdc_weth).unwrap();
        assert_eq!(slots.len(), 5);
        assert_eq!(slots[0], U256::ZERO); // slot0
    }

    #[test]
    fn unknown_address_returns_none() {
        assert!(known_slots(&Address::ZERO).is_none());
    }
}
