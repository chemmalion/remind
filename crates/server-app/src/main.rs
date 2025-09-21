// bin/server.rs (or another crate). This part is async/IO full-fat.
use tokio::{sync::mpsc, select};
use std::collections::HashMap;
use server_model::{ReminderFlow, Command, Effect, Event, Email, Document, EffId};

#[tokio::main]
async fn main() {
    // Multiple flows can be in-flight at once:
    let mut flows: HashMap<u64, ReminderFlow> = HashMap::new();
    let mut next_flow_id = 1;

    // Start one flow:
    let flow_id = { let id = next_flow_id; next_flow_id += 1; id };
    let mut flow = ReminderFlow::new("me@example.com", "is:unread", 123456789);
    let mut pending = vec![flow.start()];
    flows.insert(flow_id, flow);

    // Channels to receive completed effects back as events:
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<(u64, Event)>();

    // Kick off initial effects:
    for cmd in pending.drain(..) {
        spawn_effect(flow_id, cmd, evt_tx.clone());
    }

    // Main event loop: multiplex many flows & effects
    while let Some((fid, ev)) = evt_rx.recv().await {
        if let Some(flow) = flows.get_mut(&fid) {
            match flow.on_event(ev) {
                Command::Do(eff) => spawn_effect(fid, Command::Do(eff), evt_tx.clone()),
                Command::Wait => { /* nothing; wait for more events */ }
                Command::Done(res) => {
                    println!("flow {fid} done: {:?}", res);
                    flows.remove(&fid);
                }
            }
        }
    }
}

fn spawn_effect(fid: u64, cmd: Command, evt_tx: mpsc::UnboundedSender<(u64, Event)>) {
    match cmd {
        Command::Do(eff) => {
            match eff {
                Effect::FetchEmails { account, query, tag } => {
                    tokio::spawn(async move {
                        // call your Gmail client here
                        let emails: Vec<Email> = gmail_fetch(account, query).await;
                        let _ = evt_tx.send((fid, Event::EmailsReady { tag, emails }));
                    });
                }
                Effect::FetchDoc { doc_id, tag } => {
                    tokio::spawn(async move {
                        let doc: Document = google_docs_fetch(doc_id).await;
                        let _ = evt_tx.send((fid, Event::DocReady { tag, doc }));
                    });
                }
                Effect::SendTelegram { chat_id, text, tag } => {
                    tokio::spawn(async move {
                        telegram_send(chat_id, text).await;
                        let _ = evt_tx.send((fid, Event::TelegramDone { tag }));
                    });
                }
                Effect::SetPhoneReminder { reminder_key, when, tag } => {
                    tokio::spawn(async move {
                        iphone_api_set(reminder_key, when).await;
                        let _ = evt_tx.send((fid, Event::PhoneDone { tag }));
                    });
                }
                Effect::StartTimer { fire_at, tag } => {
                    tokio::spawn(async move {
                        let now = std::time::Instant::now();
                        if fire_at > now { tokio::time::sleep(fire_at - now).await; }
                        let _ = evt_tx.send((fid, Event::TimerFired { tag }));
                    });
                }
            }
        }
        Command::Wait | Command::Done(_) => {}
    }
}

// --- mock async calls ---
async fn gmail_fetch(_account: String, _query: String) -> Vec<Email> {
    vec![Email { id: "e1".into(), subject: "Important: Check deadline".into(), body: "doc=doc-123".into() }]
}
async fn google_docs_fetch(id: String) -> Document {
    Document { id, text: "deadline is today; please act".into() }
}
async fn telegram_send(_chat_id: i64, _text: String) {}
async fn iphone_api_set(_key: String, _when: std::time::Instant) {}
