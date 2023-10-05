use ethers::{
    abi::EncodePackedError,
    contract::ContractError,
    prelude::Middleware,
    providers::ProviderError,
    types::{TimeError, H256},
};

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    AbiError(#[from] ethers::abi::Error),

    #[error(transparent)]
    ContractAbiError(#[from] ethers::contract::AbiError),

    #[error(transparent)]
    ProviderError(#[from] ProviderError),

    #[error(transparent)]
    EncodePackedError(#[from] EncodePackedError),

    #[error(transparent)]
    TimeError(#[from] TimeError),

    #[error("Contract error {0}")]
    ContractError(String),

    #[error("Middleware error {0}")]
    Middleware(String),

    #[error("Channel is closed")]
    ChannelClosed,

    #[error("Withdrawal event with index {0} not found in transaction {1:?}")]
    WithdrawalLogNotFound(usize, H256),

    #[error("Failed to decode withdrawal event from log")]
    FailedToDecodeLog,

    #[error("WithdrawalInitiatedFilter is not found for {0:?} at index {1}")]
    WithdrawalInitiatedFilterNotFound(H256, usize),

    #[error("L2ToL1 message for transaction {0:?} with value {1:?} not found")]
    L2ToL1WithValueNotFound(H256, H256),

    #[error("L1MessageSent log not found for transaction {0:?} at index {1}")]
    L1MessageSentNotFound(H256, usize),

    #[error("Message not RLP bytes encoded: {0}")]
    MessageNotRlpBytes(String),

    #[error("Block has no number, parent block hash is {0:?}")]
    BlockHasNoNumber(H256),

    #[error("Time Conversion error in chrono")]
    TimeConversion,
}

impl<M: Middleware> From<ContractError<M>> for Error {
    fn from(value: ContractError<M>) -> Self {
        Self::ContractError(value.to_string())
    }
}

/// The client result type.
pub type Result<T> = std::result::Result<T, Error>;
