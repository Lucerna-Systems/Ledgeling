//! Common lifecycle and contracts for background subsystems.
//!
//! Subsystems (such as CodingService, Terminal, TaskService) that own threads,
//! background processes (PTYs, watchers, subprocesses) or persistent resources
//! implement [`SubsystemService`]. This guarantees that each subsystem can be
//! started, tested headlessly, and cleanly shut down without relying on global Tauri state.

pub trait SubsystemService: Send {
    /// Gracefully stops any background workers, child processes, or file watchers
    /// associated with this subsystem.
    fn shutdown(&mut self);
}

