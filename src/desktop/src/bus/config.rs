//! Configuration-arrival bus constructor and subscriber timeout.
//!
//! The [`ConfigArrived`] event payload lives in
//! [`crate::bus::events::config`]; this module owns only the
//! constructor and the timeout constant that polling subscribers use
//! when they can't block the UI thread.

use std::time::Duration;

use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;

/// Construct a fresh configuration-arrival bus.
///
/// Cloning the returned [`Bus`] is cheap and shares the same broadcast
/// channel. The bus is intentionally created by `main` (which loads the
/// config) and handed to every subsystem that needs to subscribe.
pub fn config_bus() -> Bus<ConfigArrived> {
    Bus::new()
}

/// Default timeout for subscribers that poll the bus without blocking
/// the UI thread. If the event is not observed within this window the
/// subscriber falls back to [`crate::config::AppConfig::default`] and
/// emits a `config.arrived.timeout` tracing event.
pub const CONFIG_ARRIVAL_TIMEOUT: Duration = Duration::from_millis(100);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use std::thread;

    #[test]
    fn test_publish_delivers_to_every_subscriber() {
        let bus = config_bus();
        let r1 = bus.subscribe();
        let r2 = bus.subscribe();
        let r3 = bus.subscribe();

        let delivered = bus.publish(ConfigArrived::new(AppConfig::default()));
        assert_eq!(delivered, 3);

        let event = r1.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.config.models, AppConfig::default().models);
        let event = r2.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.config.models, AppConfig::default().models);
        let event = r3.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.config.models, AppConfig::default().models);
    }

    #[test]
    fn test_subscriber_count_tracks_subscriptions() {
        let bus = config_bus();
        assert_eq!(bus.subscriber_count(), 0);
        let _a = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
        let _b = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }

    #[test]
    fn test_dropped_consumer_is_cleaned_up() {
        let bus = config_bus();
        let r1 = bus.subscribe();
        let r2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);

        drop(r1);
        assert_eq!(bus.subscriber_count(), 1);

        let delivered = bus.publish(ConfigArrived::new(AppConfig::default()));
        assert_eq!(delivered, 1);
        let _ = r2.recv_timeout(Duration::from_millis(100)).unwrap();
    }

    #[test]
    fn test_publish_with_no_subscribers_does_not_panic() {
        let bus = config_bus();
        let delivered = bus.publish(ConfigArrived::new(AppConfig::default()));
        assert_eq!(delivered, 0);
    }

    #[test]
    fn test_payload_carries_config_distinctively() {
        let bus = config_bus();
        let reader = bus.subscribe();

        let cfg = AppConfig {
            inline_editor_enabled: true,
            ..AppConfig::default()
        };
        bus.publish(ConfigArrived::new(cfg.clone()));

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(event.config.inline_editor_enabled);
        assert_eq!(event.config.csv_db_path, cfg.csv_db_path);
    }

    #[test]
    fn test_clone_shares_subscriber_list() {
        let bus = config_bus();
        let bus_clone = bus.clone();
        let reader = bus_clone.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        bus.publish(ConfigArrived::new(AppConfig::default()));
        let _ = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    }

    #[test]
    fn test_concurrent_publishers_and_subscribers() {
        let bus = config_bus();
        let mut readers = Vec::new();
        for _ in 0..4 {
            let r = bus.subscribe();
            readers.push(thread::spawn(move || {
                let event = r.recv_timeout(Duration::from_millis(500)).unwrap();
                assert!(!event.config.feature_flags.is_empty());
            }));
        }

        // Single publish: fan-out, all four consumers observe it.
        let delivered = bus.publish(ConfigArrived::new(AppConfig::default()));
        assert_eq!(delivered, 4);

        for h in readers {
            h.join().unwrap();
        }
    }
}
