//! ABI wrappers for the `L2StandardToken` contract.

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        L2StandardToken,
        "$CARGO_MANIFEST_DIR/src/contracts/IL2StandardToken.json",
    );
}
