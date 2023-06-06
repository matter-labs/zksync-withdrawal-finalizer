//! ABI wrappers for `L2Bridge` contract.

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        IL2Bridge,
        "$CARGO_MANIFEST_DIR/src/contracts/IL2Bridge.json"
    );
}
