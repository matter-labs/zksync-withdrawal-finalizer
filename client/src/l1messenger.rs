//! ABI wrappers for `L1Messenger` contract.

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        L1Messenger,
        "$CARGO_MANIFEST_DIR/src/contracts/IL1Messenger.json"
    );
}
