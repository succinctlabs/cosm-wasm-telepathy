use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Sync committee for current period is not initialized.")]
    SyncCommitteeNotInitialized {},

    #[error("Update slot is too far in the future")]
    UpdateSlotTooFar {},

    #[error("Less than MIN_SYNC_COMMITTEE_PARTICIPANTS signed.")]
    NotEnoughSyncCommitteeParticipants {},

    #[error("There exists a better update")]
    ExistsBetterUpdate {},

    #[error("Best update was never initialized")]
    BestUpdateNotInitialized {},

    #[error("Sync committee for next period already initialized.")]
    SyncCommitteeAlreadyInitialized {},

    #[error("Must wait for current sync committee period to end.")]
    CurrentSyncCommitteeNotEnded {},

    #[error("Proof failed to verify.")]
    InvalidProof {},

    #[error("Custom Error val: {val:?}")]
    CustomError { val: String },
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
