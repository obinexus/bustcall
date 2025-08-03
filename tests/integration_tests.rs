use bustcall_core::*;

#[test]
fn test_daemon_creation() {
    let daemon = Daemon::new();
    assert!(daemon.is_ok());
}

#[test]
fn test_notification_manager() {
    let manager = core::notify::NotificationManager::new();
    let result = manager.send(core::notify::NotificationLevel::Info, "Test message");
    assert!(result.is_ok());
}
