use std::process::Command;

#[test]
fn unavailable_session_bus_reports_a_startup_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_nightlight-panel"))
        .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/dev/null")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("session bus"));
}
