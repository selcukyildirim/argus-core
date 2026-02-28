//! Static label registry for well-known Ethereum contracts.
//!
//! Provides instant protocol identification without external API calls.
//! Used by the reporter module to enrich conflict reports.

use alloy_primitives::Address;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Contract metadata: protocol name and optional label.
#[derive(Debug, Clone)]
pub struct ContractLabel {
    pub protocol: &'static str,
    pub name: &'static str,
}

impl ContractLabel {
    const fn new(protocol: &'static str, name: &'static str) -> Self {
        Self { protocol, name }
    }
}

/// Returns the label for a known contract, if any.
pub fn lookup(address: &Address) -> Option<&'static ContractLabel> {
    KNOWN_LABELS.get(address)
}

static KNOWN_LABELS: LazyLock<HashMap<Address, ContractLabel>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // ── Uniswap ──────────────────────────────────────────────
    m.insert(
        addr("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"),
        ContractLabel::new("Uniswap", "V2 Router"),
    );
    m.insert(
        addr("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
        ContractLabel::new("Uniswap", "V2 Factory"),
    );
    m.insert(
        addr("0xE592427A0AEce92De3Edee1F18E0157C05861564"),
        ContractLabel::new("Uniswap", "V3 SwapRouter"),
    );
    m.insert(
        addr("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
        ContractLabel::new("Uniswap", "V3 SwapRouter02"),
    );
    m.insert(
        addr("0x1F98431c8aD98523631AE4a59f267346ea31F984"),
        ContractLabel::new("Uniswap", "V3 Factory"),
    );
    m.insert(
        addr("0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD"),
        ContractLabel::new("Uniswap", "Universal Router"),
    );
    m.insert(
        addr("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
        ContractLabel::new("Uniswap", "V2 USDC/WETH"),
    );
    m.insert(
        addr("0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852"),
        ContractLabel::new("Uniswap", "V2 WETH/USDT"),
    );
    m.insert(
        addr("0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8"),
        ContractLabel::new("Uniswap", "V3 USDC/WETH 0.3%"),
    );
    m.insert(
        addr("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
        ContractLabel::new("Uniswap", "V3 USDC/WETH 0.05%"),
    );
    m.insert(
        addr("0xCBCdF9626bC03E24f779434178A73a0B4bad62eD"),
        ContractLabel::new("Uniswap", "V3 WBTC/WETH"),
    );

    // ── Tokens ───────────────────────────────────────────────
    m.insert(
        addr("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        ContractLabel::new("WETH", "Wrapped Ether"),
    );
    m.insert(
        addr("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        ContractLabel::new("USDC", "USD Coin"),
    );
    m.insert(
        addr("0xdAC17F958D2ee523a2206206994597C13D831ec7"),
        ContractLabel::new("USDT", "Tether USD"),
    );
    m.insert(
        addr("0x6B175474E89094C44Da98b954EedeAC495271d0F"),
        ContractLabel::new("DAI", "Dai Stablecoin"),
    );
    m.insert(
        addr("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
        ContractLabel::new("WBTC", "Wrapped BTC"),
    );
    m.insert(
        addr("0x514910771AF9Ca656af840dff83E8264EcF986CA"),
        ContractLabel::new("LINK", "Chainlink Token"),
    );
    m.insert(
        addr("0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        ContractLabel::new("UNI", "Uniswap Token"),
    );
    m.insert(
        addr("0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE"),
        ContractLabel::new("SHIB", "Shiba Inu"),
    );
    m.insert(
        addr("0x7D1AfA7B718fb893dB30A3aBc0Cfc608AaCfeBB0"),
        ContractLabel::new("MATIC", "Polygon Token"),
    );
    m.insert(
        addr("0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84"),
        ContractLabel::new("stETH", "Lido Staked ETH"),
    );

    // ── Aave ─────────────────────────────────────────────────
    m.insert(
        addr("0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"),
        ContractLabel::new("Aave", "V3 Pool"),
    );
    m.insert(
        addr("0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9"),
        ContractLabel::new("Aave", "V2 LendingPool"),
    );

    // ── Curve ─────────────────────────────────────────────────
    m.insert(
        addr("0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7"),
        ContractLabel::new("Curve", "3pool"),
    );
    m.insert(
        addr("0xDC24316b9AE028F1497c275EB9192a3Ea0f67022"),
        ContractLabel::new("Curve", "stETH/ETH"),
    );

    // ── 1inch ─────────────────────────────────────────────────
    m.insert(
        addr("0x1111111254EEB25477B68fb85Ed929f73A960582"),
        ContractLabel::new("1inch", "V5 Router"),
    );
    m.insert(
        addr("0x111111125421cA6dc452d289314280a0f8842A65"),
        ContractLabel::new("1inch", "V6 Router"),
    );

    // ── OpenSea / Blur / NFT ──────────────────────────────────
    m.insert(
        addr("0x00000000000000ADc04C56Bf30aC9d3c0aAF14dC"),
        ContractLabel::new("OpenSea", "Seaport 1.5"),
    );
    m.insert(
        addr("0x00000000006c3852cbEf3e08E8dF289169EdE581"),
        ContractLabel::new("OpenSea", "Seaport 1.1"),
    );
    m.insert(
        addr("0x29469395eAf6f95920E59F858042f0e28D98a20B"),
        ContractLabel::new("Blur", "BlurPool"),
    );
    m.insert(
        addr("0x000000000000Ad05Ccc4F10045630fb830B95127"),
        ContractLabel::new("Blur", "Marketplace"),
    );
    m.insert(
        addr("0xb47e3cd837dDF8e4c57F05d70Ab865de6e193BBB"),
        ContractLabel::new("CryptoPunks", "Marketplace"),
    );

    // ── Lido ──────────────────────────────────────────────────
    m.insert(
        addr("0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84"),
        ContractLabel::new("Lido", "stETH"),
    );
    m.insert(
        addr("0x7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0"),
        ContractLabel::new("Lido", "wstETH"),
    );

    // ── EigenLayer ────────────────────────────────────────────
    m.insert(
        addr("0x858646372CC42E1A627fcE94aa7A7033e7CF075A"),
        ContractLabel::new("EigenLayer", "StrategyManager"),
    );

    // ── Gnosis Safe / Multicall ───────────────────────────────
    m.insert(
        addr("0xcA11bde05977b3631167028862bE2a173976CA11"),
        ContractLabel::new("Multicall", "Multicall3"),
    );
    m.insert(
        addr("0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552"),
        ContractLabel::new("Gnosis Safe", "SafeL2 1.3.0"),
    );

    // ── MEV ───────────────────────────────────────────────────
    m.insert(
        addr("0xC36442b4a4522E871399CD717aBDD847Ab11FE88"),
        ContractLabel::new("Uniswap", "V3 NonfungiblePositionManager"),
    );
    m.insert(
        addr("0xDef1C0ded9bec7F1a1670819833240f027b25EfF"),
        ContractLabel::new("0x Protocol", "Exchange Proxy"),
    );

    // ── MetaMask ──────────────────────────────────────────────
    m.insert(
        addr("0x881D40237659C251811CEC9c364ef91dC08D300C"),
        ContractLabel::new("MetaMask", "Swap Router"),
    );

    // ── Discovered from block 21M conflict analysis ───────────
    m.insert(
        addr("0x502Ed02100eA8b10F8d7FC14e0f86633Ec2ddada"),
        ContractLabel::new("ERC-20", "Meme Token"),
    );
    m.insert(
        addr("0x5Ae97e4770b7034C7Ca99Ab7edC26a18a23CB412"),
        ContractLabel::new("MEV Bot", "Multi-Token Aggregator"),
    );

    m
});

fn addr(s: &str) -> Address {
    s.parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_weth() {
        let weth: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
            .parse()
            .unwrap();
        let label = lookup(&weth).unwrap();
        assert_eq!(label.protocol, "WETH");
        assert_eq!(label.name, "Wrapped Ether");
    }

    #[test]
    fn unknown_returns_none() {
        assert!(lookup(&Address::ZERO).is_none());
    }
}
