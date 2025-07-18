use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{SystemTime, UNIX_EPOCH}, // AstralRinth
};

use discord_rich_presence::{
    DiscordIpc,
    DiscordIpcClient,
    activity::{Activity, Assets, Timestamps}, // AstralRinth
};
use rand::seq::SliceRandom; // AstralRinth
use tokio::sync::RwLock;

use crate::State;
use crate::util::utils; // AstralRinth

pub struct DiscordGuard {
    client: Arc<RwLock<DiscordIpcClient>>,
    connected: Arc<AtomicBool>,
}

pub(crate) const ACTIVE_STATE: [&str; 6] = [
    "Explores",
    "Travels with",
    "Pirating",
    "Investigating the",
    "Engaged in",
    "Conducting",
];
pub(crate) const INACTIVE_STATE: [&str; 6] = [
    "Idling...",
    "Waiting for the pirate team...",
    "Taking a break...",
    "Resting...",
    "On standby...",
    "In a holding pattern...",
];

impl DiscordGuard {
    /// Initialize discord IPC client, and attempt to connect to it
    /// If it fails, it will still return a DiscordGuard, but the client will be unconnected
    pub fn init() -> crate::Result<DiscordGuard> {
        let dipc =
            DiscordIpcClient::new("1190718475832918136").map_err(|e| {
                crate::ErrorKind::OtherError(format!(
                    "Could not create Discord client {e}",
                ))
            })?;

        Ok(DiscordGuard {
            client: Arc::new(RwLock::new(dipc)),
            connected: Arc::new(AtomicBool::new(false)),
        })
    }

    /// If the client failed connecting during init(), this will check for connection and attempt to reconnect
    /// This MUST be called first in any client method that requires a connection, because those can PANIC if the client is not connected
    /// (No connection is different than a failed connection, the latter will not panic and can be retried)
    pub async fn retry_if_not_ready(&self) -> bool {
        let mut client = self.client.write().await;
        if !self.connected.load(std::sync::atomic::Ordering::Relaxed) {
            if client.connect().is_ok() {
                self.connected
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
            return false;
        }
        true
    }

    /// Set the activity to the given message
    /// First checks if discord is disabled, and if so, clear the activity instead
    pub async fn set_activity(
        &self,
        msg: &str,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        // Check if discord is disabled, and if so, clear the activity instead
        let state = State::get().await?;
        let settings = crate::state::Settings::get(&state.pool).await?;
        if !settings.discord_rpc {
            Ok(self.clear_activity(true).await?)
        } else {
            Ok(self.force_set_activity(msg, reconnect_if_fail).await?)
        }
    }

    /// Sets the activity to the given message, regardless of if discord is disabled or offline
    /// Should not be used except for in the above method, or if it is already known that discord is enabled (specifically for state initialization) and we are connected to the internet
    pub async fn force_set_activity(
        &self,
        msg: &str,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        // Attempt to connect if not connected. Do not continue if it fails, as the client.set_activity can panic if it never was connected
        if !self.retry_if_not_ready().await {
            return Ok(());
        }

        // let activity = Activity::new().state(msg).assets(
        //     Assets::new()
        //         .large_image("modrinth_simple")
        //         .large_text("Modrinth Logo"),
        // );

        let launcher =
            utils::read_package_json().expect("Failed to read package.json");

        let build_info =
            format!("https://github.com/D5Kostya/smetana_app/releases/latest");
        let build_download = "test";

        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get system time")
            .as_secs() as i64;
        let activity = Activity::new()
            .state(msg)
            .assets(
                Assets::new()
                    .large_image("astralrinth_logo")
                    .large_text(&build_info),
            )
            .timestamps(Timestamps::new().start(time));

        // Attempt to set the activity
        // If the existing connection fails, attempt to reconnect and try again
        let mut client: tokio::sync::RwLockWriteGuard<'_, DiscordIpcClient> =
            self.client.write().await;
        let res = client.set_activity(activity.clone());
        let could_not_set_err = |e: Box<dyn serde::ser::StdError>| {
            crate::ErrorKind::OtherError(format!(
                "Could not update Discord activity {e}",
            ))
        };

        if reconnect_if_fail {
            if let Err(_e) = res {
                client.reconnect().map_err(|e| {
                    crate::ErrorKind::OtherError(format!(
                        "Could not reconnect to Discord IPC {e}",
                    ))
                })?;
                return Ok(client
                    .set_activity(activity)
                    .map_err(could_not_set_err)?); // try again, but don't reconnect if it fails again
            }
        } else {
            res.map_err(could_not_set_err)?;
        }

        Ok(())
    }

    /// Clear the activity entirely ('disabling' the RPC until the next set_activity)
    pub async fn clear_activity(
        &self,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        // Attempt to connect if not connected. Do not continue if it fails, as the client.clear_activity can panic if it never was connected
        if !self.retry_if_not_ready().await {
            return Ok(());
        }

        // Attempt to clear the activity
        // If the existing connection fails, attempt to reconnect and try again
        let mut client = self.client.write().await;
        let res = client.clear_activity();

        let could_not_clear_err = |e: Box<dyn serde::ser::StdError>| {
            crate::ErrorKind::OtherError(format!(
                "Could not clear Discord activity {e}",
            ))
        };

        if reconnect_if_fail {
            if res.is_err() {
                client.reconnect().map_err(|e| {
                    crate::ErrorKind::OtherError(format!(
                        "Could not reconnect to Discord IPC {e}",
                    ))
                })?;
                return Ok(client
                    .clear_activity()
                    .map_err(could_not_clear_err)?); // try again, but don't reconnect if it fails again
            }
        } else {
            res.map_err(could_not_clear_err)?;
        }
        Ok(())
    }

    /// Clear the activity, but if there is a running profile, set the activity to that instead
    pub async fn clear_to_default(
        &self,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        let state = State::get().await?;

        let settings = crate::state::Settings::get(&state.pool).await?;
        if !settings.discord_rpc {
            println!("Discord is disabled, clearing activity");
            return self.clear_activity(true).await;
        }

        let selected_phrase =
            INACTIVE_STATE.choose(&mut rand::thread_rng()).unwrap();
        self.set_activity(&format!("{}", selected_phrase), reconnect_if_fail)
            .await?;
        Ok(())
    }
}
