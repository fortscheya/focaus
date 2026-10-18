//! Validate the raw IPC value before serde can turn a missing nullable field into None.
use focaus_contracts::{Command, CommandName, CommandResult, CommandResultStatus, SafeError};
use serde_json::Value;

pub fn decode_command(wire: Value) -> Result<Command, SafeError> {
    focaus_contracts::validate("Command", &wire)?;
    serde_json::from_value(wire).map_err(|_| {
        focaus_contracts::error(
            focaus_contracts::ErrorCode::InvalidArgument,
            "Invalid command wire value",
        )
    })
}
pub fn validate_result(result: &CommandResult) -> Result<(), SafeError> {
    let wire = serde_json::to_value(result).map_err(|_| {
        focaus_contracts::error(
            focaus_contracts::ErrorCode::InvalidResult,
            "Invalid response wire value",
        )
    })?;
    focaus_contracts::validate("CommandResult", &wire)
}
/// Validate the concrete result as well as its envelope before crossing IPC.
pub fn validate_command_result(
    name: &CommandName,
    result: &CommandResult,
) -> Result<(), SafeError> {
    validate_result(result)?;
    if result.status != CommandResultStatus::Ok {
        return Ok(());
    }
    let name = serde_json::to_value(name).map_err(|_| {
        focaus_contracts::error(
            focaus_contracts::ErrorCode::InvalidResult,
            "Invalid command name",
        )
    })?;
    let catalogue = focaus_contracts::command_catalogue();
    let kind = catalogue
        .get(name.as_str().unwrap_or(""))
        .and_then(|v| v["output"].as_str())
        .ok_or_else(|| {
            focaus_contracts::error(
                focaus_contracts::ErrorCode::InvalidResult,
                "Unknown result type",
            )
        })?;
    let body = result.result.as_ref().ok_or_else(|| {
        focaus_contracts::error(
            focaus_contracts::ErrorCode::InvalidResult,
            "Successful command omitted its result",
        )
    })?;
    focaus_contracts::validate(kind, body)
}
