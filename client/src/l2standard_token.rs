//! ABI wrappers for the `L2StandardToken` contract.

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        L2StandardToken,
        "$CARGO_MANIFEST_DIR/src/contracts/IL2StandardToken.json",
    );

    // The name of the event was changed in
    // https://github.com/matter-labs/zksync-2-contracts/commit/ef3517270f0a38928a25976e39eb03a1c92d07ae
    abigen!(
        OldL2StandardToken,
        r#"[
            event BridgeInitialization(address indexed l1Token, string name, string symbol, uint8 decimals)
        ]"#
    );

    impl From<BridgeInitializationFilter> for BridgeInitializeFilter {
        fn from(value: BridgeInitializationFilter) -> Self {
            BridgeInitializeFilter {
                l_1_token: value.l_1_token,
                name: value.name,
                symbol: value.symbol,
                decimals: value.decimals,
            }
        }
    }
}
