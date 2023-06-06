//! ABI wrappers for the `IEthToken` contract.

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(EthToken, "$CARGO_MANIFEST_DIR/src/contracts/IEthToken.json",);
}
