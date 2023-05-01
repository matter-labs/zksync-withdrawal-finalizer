//! ABI wrappers for `L1Messenger` contract.

pub use codegen::L1MessageSentFilter;

#[allow(missing_docs)]
mod codegen {
    use ethers::prelude::abigen;

    abigen!(L1Messenger, "./src/contracts/IL1Messenger.json");
}
