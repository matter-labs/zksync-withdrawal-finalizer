//! ABI wrappers for `WithdrawalFinalizer` contract.

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        WithdrawalFinalizer,
        "$CARGO_MANIFEST_DIR/src/contracts/WithdrawalFinalizer.json"
    );
}
