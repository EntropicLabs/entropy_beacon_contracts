use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Key already whitelisted")]
    KeyAlreadyWhitelisted {},

    #[error("Key not whitelisted")]
    KeyNotWhitelisted {},

    #[error("Key not active yet (activates at height {activation_height})")]
    KeyNotActive { activation_height: u64 },

    #[error("Invalid public key")]
    InvalidPublicKey {},

    #[error("Invalid VRF proof")]
    InvalidProof {},

    #[error("Invalid message: must be the same as the last submitted entropy")]
    InvalidMessage {},

    #[error("Hex parse error")]
    HexParseError {},

    #[error("Insufficient funds")]
    InsufficientFunds {},

    #[error("Invalid reply ID")]
    InvalidReplyId {},

    #[error("No active requests")]
    NoActiveRequests {},
}
