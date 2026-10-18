//! Validate the raw IPC value before serde can turn a missing nullable field into None.
use focaus_contracts::{Command, CommandResult, SafeError};
use serde_json::Value;

pub fn decode_command(wire: Value) -> Result<Command, SafeError> {
    focaus_contracts::validate("Command", &wire)?;
    serde_json::from_value(wire).map_err(|_| focaus_contracts::error(
        focaus_contracts::ErrorCode::InvalidArgument, "Invalid command wire value"))
}
pub fn validate_result(result: &CommandResult) -> Result<(), SafeError> {
    let wire = serde_json::to_value(result).map_err(|_| focaus_contracts::error(
        focaus_contracts::ErrorCode::InvalidResult, "Invalid response wire value"))?;
    focaus_contracts::validate("CommandResult", &wire)
}
