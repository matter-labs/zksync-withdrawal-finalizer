//! ABI wrappers for `L1Bridge` contract.

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        IL1Bridge,
        "$CARGO_MANIFEST_DIR/src/contracts/IL1Bridge.json"
    );
}
