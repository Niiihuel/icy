use std::{collections::HashMap, time::Duration};

use zbus::{
    blocking::{Connection, Proxy, connection::Builder},
    zvariant::Value,
};

use super::DesktopNotification;

#[derive(Default)]
pub(super) struct Notifier {
    connection: Option<Connection>,
}

impl Notifier {
    pub(super) fn show(&mut self, notification: &DesktopNotification) -> zbus::Result<()> {
        if self.connection.is_none() {
            self.connection = Some(
                Builder::session()?
                    .method_timeout(Duration::from_secs(3))
                    .build()?,
            );
        }
        let result = send(self.connection.as_ref().unwrap(), notification);
        if result.is_err() {
            self.connection = None;
        }
        result
    }
}

fn send(connection: &Connection, notice: &DesktopNotification) -> zbus::Result<()> {
    const SERVICE: &str = "org.freedesktop.Notifications";
    let proxy = Proxy::new(
        connection,
        SERVICE,
        "/org/freedesktop/Notifications",
        SERVICE,
    )?;
    let urgency = if notice.needs_input { 2_u8 } else { 1 };
    let hints = HashMap::from([("urgency", Value::from(urgency))]);
    let body = notice
        .body
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let _: u32 = proxy.call(
        "Notify",
        &(
            "icy",
            0_u32,
            "utilities-terminal",
            &notice.title,
            body,
            Vec::<&str>::new(),
            hints,
            -1_i32,
        ),
    )?;
    Ok(())
}
