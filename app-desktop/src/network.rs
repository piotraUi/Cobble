//! Runs the `protocol` connection on its own OS thread with a
//! single-threaded Tokio runtime, and bridges it to the winit main
//! loop over plain channels so `main.rs` never has to `await` anything.

use protocol::GameEvent;
use tokio::sync::mpsc;

pub struct OutgoingPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

pub struct NetworkHandle {
    pub events: mpsc::UnboundedReceiver<GameEvent>,
    position_tx: mpsc::UnboundedSender<OutgoingPosition>,
}

impl NetworkHandle {
    /// Queues a position update to be sent to the server. Safe to call
    /// from the winit main thread every frame — cheap, non-blocking.
    pub fn send_position(&self, pos: OutgoingPosition) {
        let _ = self.position_tx.send(pos);
    }
}

pub fn connect(host: String, port: u16, username: String) -> NetworkHandle {
    let (events_tx, events_rx) = mpsc::unbounded_channel::<GameEvent>();
    let (position_tx, mut position_rx) = mpsc::unbounded_channel::<OutgoingPosition>();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build network runtime");

        runtime.block_on(async move {
            let mut conn = match protocol::MinecraftConnection::connect(&host, port, &username).await {
                Ok(conn) => conn,
                Err(e) => {
                    log::error!("failed to connect to {host}:{port}: {e}");
                    let _ = events_tx.send(GameEvent::Disconnected(format!("{e}")));
                    return;
                }
            };
            log::info!("connected to {host}:{port} as {username}");

            loop {
                tokio::select! {
                    event = conn.next_event() => {
                        match event {
                            Ok(event) => {
                                let disconnected = matches!(event, GameEvent::Disconnected(_));
                                if events_tx.send(event).is_err() || disconnected {
                                    return;
                                }
                            }
                            Err(e) => {
                                log::error!("connection error: {e}");
                                let _ = events_tx.send(GameEvent::Disconnected(format!("{e}")));
                                return;
                            }
                        }
                    }
                    Some(pos) = position_rx.recv() => {
                        if let Err(e) = conn
                            .send_player_position_and_look(pos.x, pos.y, pos.z, pos.yaw, pos.pitch, pos.on_ground)
                            .await
                        {
                            log::error!("failed to send position update: {e}");
                            return;
                        }
                    }
                }
            }
        });
    });

    NetworkHandle {
        events: events_rx,
        position_tx,
    }
}
