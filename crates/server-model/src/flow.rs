// lib.rs (your model crate) — no async, no IO, no external deps.
use std::time::{Duration, Instant};

// --- Domain placeholders (pure data) ---
#[derive(Debug, Clone)]
pub struct Email { pub id: String, pub subject: String, pub body: String }

#[derive(Debug, Clone)]
pub struct Document { pub id: String, pub text: String }

// Correlates an effect with the flow that issued it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffId(u64);

// What the model wants the outer world to do next (an “effect”).
#[derive(Debug, Clone)]
pub enum Effect {
    FetchEmails { account: String, query: String, tag: EffId },
    FetchDoc    { doc_id: String,                   tag: EffId },
    SendTelegram{ chat_id: i64, text: String,       tag: EffId },
    SetPhoneReminder { reminder_key: String, when: Instant, tag: EffId },
    StartTimer  { fire_at: Instant,                 tag: EffId },
}

// Results coming back in (an “event”).
#[derive(Debug, Clone)]
pub enum Event {
    EmailsReady    { tag: EffId, emails: Vec<Email> },
    DocReady       { tag: EffId, doc: Document },
    TelegramDone   { tag: EffId },
    PhoneDone      { tag: EffId },
    TimerFired     { tag: EffId },
    Failed         { tag: EffId, error: String },
}

// What a flow “says” after each step.
#[derive(Debug, Clone)]
pub enum Command {
    Do(Effect),              // please perform this
    Wait,                    // waiting for a matching Event
    Done(Result<(), String>) // finished
}

// One flow instance with tiny internal state.
pub struct ReminderFlow {
    stage: Stage,
    acc: String,
    query: String,
    chat_id: i64,
    next_tag: u64,
}

#[derive(Debug, Clone)]
enum Stage {
    Start,
    WaitingEmails { tag: EffId },
    WaitingDoc    { tag: EffId, doc_id: String },
    WaitingTelegram { tag: EffId },
    WaitingPhone    { tag: EffId },
    Done(Result<(), String>),
}

impl ReminderFlow {
    pub fn new(account: impl Into<String>, query: impl Into<String>, chat_id: i64) -> Self {
        Self {
            stage: Stage::Start,
            acc: account.into(),
            query: query.into(),
            chat_id,
            next_tag: 1,
        }
    }

    fn next(&mut self) -> EffId {
        let id = self.next_tag;
        self.next_tag += 1;
        EffId(id)
    }

    // Start or continue without any incoming event.
    pub fn start(&mut self) -> Command {
        match self.stage {
            Stage::Start => {
                let tag = self.next();
                self.stage = Stage::WaitingEmails { tag };
                Command::Do(Effect::FetchEmails {
                    account: self.acc.clone(),
                    query:   self.query.clone(),
                    tag
                })
            }
            Stage::Done(ref res) => Command::Done(res.clone()),
            _ => Command::Wait,
        }
    }

    // Feed results back into the flow; it advances deterministically.
    pub fn on_event(&mut self, ev: Event) -> Command {
        match (&mut self.stage, ev) {
            // 1) Got emails?
            (Stage::WaitingEmails { tag }, Event::EmailsReady { tag: t, emails }) if *tag == t => {
                // Example model logic: if we find a “Important:” email, fetch a doc it references
                if let Some(doc_id) = pick_interesting_doc(&emails) {
                    let t2 = self.next();
                    self.stage = Stage::WaitingDoc { tag: t2, doc_id: doc_id.clone() };
                    Command::Do(Effect::FetchDoc { doc_id, tag: t2 })
                } else {
                    self.stage = Stage::Done(Ok(()));
                    Command::Done(Ok(()))
                }
            }

            // 2) Got the doc? Decide on Telegram reminder.
            (Stage::WaitingDoc { tag, .. }, Event::DocReady { tag: t, doc }) if *tag == t => {
                if should_notify(&doc) {
                    let t2 = self.next();
                    self.stage = Stage::WaitingTelegram { tag: t2 };
                    Command::Do(Effect::SendTelegram {
                        chat_id: self.chat_id,
                        text: format!("Heads up: {}", summarize(&doc)),
                        tag: t2
                    })
                } else {
                    self.stage = Stage::Done(Ok(()));
                    Command::Done(Ok(()))
                }
            }

            // 3) Telegram confirmed? Schedule iPhone reminder.
            (Stage::WaitingTelegram { tag }, Event::TelegramDone { tag: t }) if *tag == t => {
                let when = Instant::now() + Duration::from_secs(3600);
                let t2 = self.next();
                self.stage = Stage::WaitingPhone { tag: t2 };
                Command::Do(Effect::SetPhoneReminder {
                    reminder_key: "important-email".to_string(),
                    when,
                    tag: t2
                })
            }

            // 4) Phone done? Finish.
            (Stage::WaitingPhone { tag }, Event::PhoneDone { tag: t }) if *tag == t => {
                self.stage = Stage::Done(Ok(()));
                Command::Done(Ok(()))
            }

            // Timers or failures (example):
            (_, Event::Failed { error, .. }) => {
                self.stage = Stage::Done(Err(error));
                Command::Done(self.done().clone())
            }
            (_, Event::TimerFired { .. }) => Command::Wait,

            // Irrelevant event for current stage:
            _ => Command::Wait,
        }
    }

    pub fn done(&self) -> &Result<(), String> {
        match &self.stage {
            Stage::Done(res) => res,
            _ => &Ok(())
        }
    }
}

// --- Tiny, pure helpers (your domain logic stays here) ---
fn pick_interesting_doc(emails: &[Email]) -> Option<String> {
    emails.iter()
        .find(|e| e.subject.contains("Important:"))
        .map(|e| extract_doc_id(&e.body))
}

fn extract_doc_id(_body: &str) -> String { "doc-123".into() }
fn should_notify(doc: &Document) -> bool { doc.text.contains("deadline") }
fn summarize(doc: &Document) -> String { doc.text.chars().take(60).collect() }
