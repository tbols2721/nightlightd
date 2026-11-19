//! Single instance: the panel claims a D-Bus name the way the daemon does
//! (#19), so a second "Settings…" click never opens a second window. If a panel
//! is already running, the new process asks it to raise itself and then exits.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::{interface, proxy};

const NAME: &str = "org.nightlightd.Panel";
const PATH: &str = "/org/nightlightd/Panel";

/// Serves `Present`: sets a flag the egui loop watches so it raises the window.
struct Presenter {
    focus: Arc<AtomicBool>,
}

#[interface(name = "org.nightlightd.Panel")]
impl Presenter {
    /// Asks the running panel to come to the front.
    fn present(&self) {
        self.focus.store(true, Ordering::Relaxed);
    }
}

/// A blocking proxy for an already-running panel's `Present`.
#[proxy(
    interface = "org.nightlightd.Panel",
    default_service = "org.nightlightd.Panel",
    default_path = "/org/nightlightd/Panel"
)]
trait Panel {
    fn present(&self) -> zbus::Result<()>;
}

/// Claims the panel name. Returns `Some(conn)` when this is the only panel —
/// keep the connection alive for the process's lifetime — or `None` when one is
/// already running, in which case that one is asked to raise itself and this
/// process should exit. `focus` is set (on the owner) whenever `Present` is
/// called; the egui loop clears it and focuses the window.
///
/// Errors when the session bus cannot be reached, or when another panel holds
/// the name and cannot be asked to come forward.
pub fn acquire(focus: Arc<AtomicBool>) -> zbus::Result<Option<Connection>> {
    let connection = Builder::session()?
        .serve_at(PATH, Presenter { focus })?
        .build()?;
    if claim(&connection)? {
        return Ok(Some(connection));
    }
    // Someone holds the name. If they cannot be reached, they let go of it
    // between our request and our call — a panel closed a moment before this
    // one opened — so the name is most likely free now: ask once more.
    if PanelProxyBlocking::new(&connection)?.present().is_ok() {
        return Ok(None);
    }
    if claim(&connection)? {
        return Ok(Some(connection));
    }
    // Taken again in that instant: a different panel won the race. Raise it,
    // and this time a failure is a real one.
    PanelProxyBlocking::new(&connection)?.present()?;
    Ok(None)
}

/// Asks for the panel name once, without queueing. `Ok(true)` means it is
/// ours; `Ok(false)` means another panel holds it.
fn claim(connection: &Connection) -> zbus::Result<bool> {
    match connection.request_name_with_flags(NAME, RequestNameFlags::DoNotQueue.into()) {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => Ok(true),
        Ok(RequestNameReply::Exists) | Err(zbus::Error::NameTaken) => Ok(false),
        Ok(RequestNameReply::InQueue) => Err(zbus::Error::Failure(
            "unexpectedly queued for the panel bus name".into(),
        )),
        Err(error) => Err(error),
    }
}
