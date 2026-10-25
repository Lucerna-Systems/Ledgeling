//! Idle time on Wayland, where the X server only sees input aimed at XWayland
//! apps. Both sources below are offered to ordinary clients without any
//! permission prompt; where neither exists, idle time is reported unavailable.
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// How long without input before the compositor says "idle". Short, so the
/// reported idle time is never more than this far behind the truth.
const NOTIFY_AFTER: Duration = Duration::from_secs(1);

type IdleSince = Arc<Mutex<Option<Instant>>>;

pub enum WaylandIdle {
    /// `ext-idle-notify-v1`: KWin, wlroots compositors and newer Mutter.
    Notify(IdleSince),
    /// GNOME Shell's idle monitor on the session bus.
    Mutter(Mutex<dbus::blocking::Connection>),
    Unavailable,
}

impl WaylandIdle {
    pub fn connect() -> Self {
        if let Some(since) = notify::start() {
            return Self::Notify(since);
        }
        match mutter::connect() {
            Some(connection) => Self::Mutter(Mutex::new(connection)),
            None => Self::Unavailable,
        }
    }

    pub fn seconds(&self) -> Result<f64, String> {
        match self {
            Self::Notify(since) => Ok(since
                .lock()
                .map_err(|_| "Idle state is unavailable")?
                .map(|at| at.elapsed().as_secs_f64())
                .unwrap_or(0.)),
            Self::Mutter(connection) => {
                let connection = connection.lock().map_err(|_| "Idle state is unavailable")?;
                mutter::idle_seconds(&connection)
            }
            Self::Unavailable => {
                Err("This Wayland compositor offers no idle time without extra permissions.".into())
            }
        }
    }
}

mod notify {
    use super::{IdleSince, NOTIFY_AFTER};
    use std::{
        sync::{Arc, Mutex},
        time::Instant,
    };
    use wayland_client::{
        delegate_noop,
        protocol::{wl_registry, wl_seat},
        Connection, Dispatch, QueueHandle,
    };
    use wayland_protocols::ext::idle_notify::v1::client::{
        ext_idle_notification_v1::{self, ExtIdleNotificationV1},
        ext_idle_notifier_v1::ExtIdleNotifierV1,
    };

    struct State {
        seat: Option<wl_seat::WlSeat>,
        notifier: Option<(ExtIdleNotifierV1, u32)>,
        since: IdleSince,
    }

    impl Dispatch<wl_registry::WlRegistry, ()> for State {
        fn event(
            state: &mut Self,
            registry: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            queue: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global {
                name,
                interface,
                version,
            } = event
            {
                match interface.as_str() {
                    "wl_seat" if state.seat.is_none() => {
                        state.seat = Some(registry.bind(name, 1, queue, ()));
                    }
                    "ext_idle_notifier_v1" => {
                        let version = version.min(2);
                        state.notifier = Some((registry.bind(name, version, queue, ()), version));
                    }
                    _ => {}
                }
            }
        }
    }

    impl Dispatch<ExtIdleNotificationV1, ()> for State {
        fn event(
            state: &mut Self,
            _: &ExtIdleNotificationV1,
            event: ext_idle_notification_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            let Ok(mut since) = state.since.lock() else {
                return;
            };
            match event {
                ext_idle_notification_v1::Event::Idled => {
                    *since = Some(
                        Instant::now()
                            .checked_sub(NOTIFY_AFTER)
                            .unwrap_or_else(Instant::now),
                    );
                }
                ext_idle_notification_v1::Event::Resumed => *since = None,
                _ => {}
            }
        }
    }

    delegate_noop!(State: ignore wl_seat::WlSeat);
    delegate_noop!(State: ExtIdleNotifierV1);

    /// The compositor pushes idle/resumed transitions to a thread that sleeps
    /// in between, so polling costs nothing.
    pub fn start() -> Option<IdleSince> {
        let connection = Connection::connect_to_env().ok()?;
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        connection.display().get_registry(&handle, ());
        let since: IdleSince = Arc::new(Mutex::new(None));
        let mut state = State {
            seat: None,
            notifier: None,
            since: since.clone(),
        };
        queue.roundtrip(&mut state).ok()?;
        let seat = state.seat.clone()?;
        let (notifier, version) = state.notifier.clone()?;
        let timeout = NOTIFY_AFTER.as_millis() as u32;
        // Version 2 counts input only; version 1 also honours idle inhibitors,
        // which would call someone watching a film not idle.
        let notification = if version >= 2 {
            notifier.get_input_idle_notification(timeout, &seat, &handle, ())
        } else {
            notifier.get_idle_notification(timeout, &seat, &handle, ())
        };
        queue.roundtrip(&mut state).ok()?;
        std::thread::Builder::new()
            .name("perch-idle".into())
            .spawn(move || {
                let _notification = notification;
                while queue.blocking_dispatch(&mut state).is_ok() {}
            })
            .ok()?;
        Some(since)
    }
}

mod mutter {
    use std::time::Duration;

    pub fn connect() -> Option<dbus::blocking::Connection> {
        let connection = dbus::blocking::Connection::new_session().ok()?;
        idle_seconds(&connection).ok()?;
        Some(connection)
    }

    pub fn idle_seconds(connection: &dbus::blocking::Connection) -> Result<f64, String> {
        let proxy = connection.with_proxy(
            "org.gnome.Mutter.IdleMonitor",
            "/org/gnome/Mutter/IdleMonitor/Core",
            Duration::from_millis(250),
        );
        let (milliseconds,): (u64,) = proxy
            .method_call("org.gnome.Mutter.IdleMonitor", "GetIdletime", ())
            .map_err(|e| format!("GNOME did not report idle time ({e})"))?;
        Ok(milliseconds as f64 / 1000.)
    }
}
