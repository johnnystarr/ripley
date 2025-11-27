use ripley::api::{ApiEvent, ApiState, RipStatus};
use ripley::config::Config;
use ripley::database::Database;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Helper to create test API state
fn create_test_state() -> ApiState {
    let (event_tx, _) = broadcast::channel(100);
    let db = Arc::new(Database::new().unwrap());
    ApiState {
        config: Arc::new(RwLock::new(Config::default())),
        rip_status: Arc::new(RwLock::new(RipStatus::default())),
        event_tx,
        db,
    }
}

#[tokio::test]
async fn test_api_state_creation() {
    let state = create_test_state();
    let status = state.rip_status.read().await;
    assert!(status.active_rips.is_empty());
    assert!(status.logs.is_empty());
}

#[tokio::test]
async fn test_rip_status_default() {
    let status = RipStatus::default();
    assert!(status.active_rips.is_empty());
    assert!(status.logs.is_empty());
}

#[tokio::test]
async fn test_api_event_serialization() {
    let events = vec![
        ApiEvent::RipStarted {
            disc: "Test Disc".to_string(),
            drive: "/dev/disk2".to_string(),
        },
        ApiEvent::RipProgress {
            progress: 0.5,
            message: "Processing...".to_string(),
            drive: "/dev/disk2".to_string(),
        },
        ApiEvent::RipCompleted {
            disc: "Test Disc".to_string(),
            drive: "/dev/disk2".to_string(),
        },
        ApiEvent::RipError {
            error: "Test error".to_string(),
            drive: Some("/dev/disk2".to_string()),
        },
        ApiEvent::Log {
            level: "info".to_string(),
            message: "Test log".to_string(),
            drive: None,
        },
    ];

    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("type") || json.contains("RipStarted") || json.contains("RipProgress"));
        
        // Deserialize back
        let deserialized: ApiEvent = serde_json::from_str(&json).unwrap();
        // Just verify it deserializes without error
        match deserialized {
            ApiEvent::RipStarted { .. } => {}
            ApiEvent::RipProgress { .. } => {}
            ApiEvent::RipCompleted { .. } => {}
            ApiEvent::RipError { .. } => {}
            ApiEvent::Log { .. } => {}
            ApiEvent::StatusUpdate { .. } => {}
            ApiEvent::DriveDetected { .. } => {}
            ApiEvent::DriveRemoved { .. } => {}
            ApiEvent::DriveEjected { .. } => {}
            ApiEvent::IssueCreated { .. } => {}
            ApiEvent::RipPaused { .. } => {}
            ApiEvent::RipResumed { .. } => {}
        }
    }
}

#[tokio::test]
async fn test_broadcast_channel() {
    let state = create_test_state();
    let mut rx1 = state.event_tx.subscribe();
    let mut rx2 = state.event_tx.subscribe();

    let event = ApiEvent::Log {
        level: "info".to_string(),
        message: "Test broadcast".to_string(),
        drive: None,
    };

    state.event_tx.send(event).unwrap();

    // Both receivers should get the event
    let received1 = rx1.recv().await.unwrap();
    let received2 = rx2.recv().await.unwrap();

    if let ApiEvent::Log { message: msg1, .. } = received1 {
        assert_eq!(msg1, "Test broadcast");
    } else {
        panic!("Wrong event type received");
    }

    if let ApiEvent::Log { message: msg2, .. } = received2 {
        assert_eq!(msg2, "Test broadcast");
    } else {
        panic!("Wrong event type received");
    }
}

#[tokio::test]
async fn test_config_read_write() {
    let state = create_test_state();

    // Read initial config
    {
        let config = state.config.read().await;
        assert!(config.speech_match.enabled);
    }

    // Write new config
    {
        let mut config = state.config.write().await;
        config.speech_match.enabled = false;
    }

    // Verify change
    {
        let config = state.config.read().await;
        assert!(!config.speech_match.enabled);
    }
}

#[tokio::test]
async fn test_rip_status_updates() {
    let state = create_test_state();

    // Initial state
    {
        let status = state.rip_status.read().await;
        assert!(status.active_rips.is_empty());
        assert!(status.logs.is_empty());
    }

    // Start ripping
    {
        let mut status = state.rip_status.write().await;
        status.active_rips.insert("/dev/disk2".to_string(), ripley::api::DriveRipStatus {
            current_disc: Some("Test Disc".to_string()),
            current_title: None,
            progress: 0.0,
            paused: false,
            paused_at: None,
        });
    }

    // Update progress
    {
        let mut status = state.rip_status.write().await;
        if let Some(drive_status) = status.active_rips.get_mut("/dev/disk2") {
            drive_status.progress = 0.5;
        }
        status.logs.push("Processing title 1".to_string());
    }

    // Complete ripping
    {
        let mut status = state.rip_status.write().await;
        status.active_rips.remove("/dev/disk2");
        status.logs.push("Ripping complete".to_string());
    }

    // Verify final state
    {
        let status = state.rip_status.read().await;
        assert!(status.active_rips.is_empty());
        assert_eq!(status.logs.len(), 2);
    }
}

#[tokio::test]
async fn test_status_update_event() {
    let mut status = RipStatus::default();
    status.active_rips.insert("/dev/disk2".to_string(), ripley::api::DriveRipStatus {
        current_disc: Some("Test".to_string()),
        current_title: Some("Title 1".to_string()),
        progress: 0.75,
        paused: false,
        paused_at: None,
    });
    status.logs.push("Log 1".to_string());
    status.logs.push("Log 2".to_string());

    let event = ApiEvent::StatusUpdate {
        status: status.clone(),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: ApiEvent = serde_json::from_str(&json).unwrap();

    if let ApiEvent::StatusUpdate { status: s } = deserialized {
        assert_eq!(s.active_rips.len(), status.active_rips.len());
        assert_eq!(s.logs.len(), status.logs.len());
    } else {
        panic!("Wrong event type");
    }
}

#[tokio::test]
async fn test_multiple_log_events() {
    let state = create_test_state();
    let mut rx = state.event_tx.subscribe();

    // Send multiple events
    for i in 1..=5 {
        let event = ApiEvent::Log {
            level: "info".to_string(),
            message: format!("Log message {}", i),
            drive: None,
        };
        state.event_tx.send(event).unwrap();
    }

    // Receive and verify all events
    for i in 1..=5 {
        let received = rx.recv().await.unwrap();
        if let ApiEvent::Log { message, .. } = received {
            assert_eq!(message, format!("Log message {}", i));
        } else {
            panic!("Wrong event type");
        }
    }
}

#[tokio::test]
async fn test_rip_status_serialization() {
    let mut status = RipStatus::default();
    status.active_rips.insert("/dev/disk2".to_string(), ripley::api::DriveRipStatus {
        current_disc: Some("Futurama Season 1".to_string()),
        current_title: Some("Space Pilot 3000".to_string()),
        progress: 0.42,
        paused: false,
        paused_at: None,
    });
    status.logs.push("Starting rip...".to_string());
    status.logs.push("Processing MKV...".to_string());

    let json = serde_json::to_string(&status).unwrap();
    let deserialized: RipStatus = serde_json::from_str(&json).unwrap();

    assert_eq!(status.active_rips.len(), deserialized.active_rips.len());
    assert_eq!(status.logs.len(), deserialized.logs.len());
    assert_eq!(status.logs, deserialized.logs);
}

#[tokio::test]
async fn test_config_clone() {
    let state = create_test_state();
    let config1 = state.config.read().await.clone();
    let config2 = state.config.read().await.clone();

    assert_eq!(config1.speech_match.enabled, config2.speech_match.enabled);
    assert_eq!(
        config1.filebot.skip_by_default,
        config2.filebot.skip_by_default
    );
}
