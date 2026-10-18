use focaus_desktop::wire::decode_command;
use serde_json::json;

#[test]
fn wire_requires_explicit_nullable_identity_and_rejects_unknown_fields() {
    let wire = json!({"contract_version":"2.0.0", "command_id":"00000000-0000-4000-8000-000000000001", "name":"state.close", "actor":"user", "window_id":null, "payload":{"subscription_id":"00000000-0000-4000-8000-000000000002"}});
    assert!(decode_command(wire.clone()).is_ok());
    let mut omitted = wire.clone(); omitted.as_object_mut().unwrap().remove("window_id");
    assert!(decode_command(omitted).is_err());
    let mut unknown = wire; unknown["privileged"] = json!(true);
    assert!(decode_command(unknown).is_err());
}
