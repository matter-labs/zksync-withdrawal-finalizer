//! ABI wrappers for `ContractDeployer` contract.

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        ContractDeployer,
        "$CARGO_MANIFEST_DIR/src/contracts/ContractDeployer.json"
    );
}
