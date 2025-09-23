#![allow(dead_code)]

#[cfg(test)]
mod tests {
    use server_model::{Command, Effect, EmailField, Event, ReminderFlow, CONFIG_DISCOVERY};

    fn config_yaml() -> String {
        r#"
run_every: "10m"
credentials:
  google_docs:
    env: GOOGLE_DOCS_TOKEN
  telegram_bot:
    env: TELEGRAM_BOT_TOKEN
steps:
  - type: google_sheet
    sheet_id:
      env: SHEET_ID
    cell:
      row: 2
      column: 3
    store_as: sheet_value
    credentials: google_docs
  - type: email
    account: "alerts@example.com"
    field: subject
    regex: "Alert {{sheet_value}}"
    store_as: email_subject
  - type: telegram
    chat_id: "@channel"
    message: "We saw {{email_subject}}"
    credentials: telegram_bot
"#
        .to_string()
    }

    #[test]
    fn flow_runs_through_steps() {
        let mut flow = ReminderFlow::new();
        let cmd = flow.start();
        let tag = match cmd {
            Command::Do(Effect::LoadConfig { discovery, tag }) => {
                assert_eq!(discovery.env_var, CONFIG_DISCOVERY.env_var);
                assert_eq!(discovery.fallback_paths, CONFIG_DISCOVERY.fallback_paths);
                tag
            }
            other => panic!("unexpected command: {:?}", other),
        };

        let cmd = flow.on_event(Event::ConfigLoaded {
            tag,
            path: "/tmp/remind.yaml".into(),
            contents: config_yaml(),
        });

        let env_tag = match cmd {
            Command::Do(Effect::ReadEnvVar { name, tag }) => {
                assert_eq!(name, "GOOGLE_DOCS_TOKEN");
                tag
            }
            other => panic!("expected env read, got {:?}", other),
        };

        let cmd = flow.on_event(Event::EnvVarLoaded {
            tag: env_tag,
            name: "GOOGLE_DOCS_TOKEN".into(),
            value: Some("docs-cred".into()),
        });

        let env_tag = match cmd {
            Command::Do(Effect::ReadEnvVar { name, tag }) => {
                assert_eq!(name, "SHEET_ID");
                tag
            }
            other => panic!("expected env read, got {:?}", other),
        };

        let cmd = flow.on_event(Event::EnvVarLoaded {
            tag: env_tag,
            name: "SHEET_ID".into(),
            value: Some("sheet-123".into()),
        });

        let env_tag = match cmd {
            Command::Do(Effect::ReadEnvVar { name, tag }) => {
                assert_eq!(name, "TELEGRAM_BOT_TOKEN");
                tag
            }
            other => panic!("expected env read, got {:?}", other),
        };

        let cmd = flow.on_event(Event::EnvVarLoaded {
            tag: env_tag,
            name: "TELEGRAM_BOT_TOKEN".into(),
            value: Some("tg-token".into()),
        });

        let sheet_tag = match cmd {
            Command::Do(Effect::FetchGoogleSheetCell(request)) => {
                assert_eq!(request.sheet_id, "sheet-123");
                assert_eq!(request.cell.row, 2);
                assert_eq!(request.cell.column, 3);
                assert_eq!(request.credentials.as_deref(), Some("docs-cred"));
                request.tag
            }
            other => panic!("expected google sheet request, got {:?}", other),
        };

        let cmd = flow.on_event(Event::StepCompleted {
            tag: sheet_tag,
            value: Some("2024-05-01".into()),
        });

        let email_tag = match cmd {
            Command::Do(Effect::SearchEmails(request)) => {
                assert_eq!(request.account, "alerts@example.com");
                assert_eq!(request.field, EmailField::Subject);
                assert_eq!(request.regex, "Alert 2024-05-01");
                assert!(request.credentials.is_none());
                request.tag
            }
            other => panic!("expected email search, got {:?}", other),
        };

        let cmd = flow.on_event(Event::StepCompleted {
            tag: email_tag,
            value: Some("Alert 2024-05-01".into()),
        });

        let telegram_tag = match cmd {
            Command::Do(Effect::SendTelegramMessage(request)) => {
                assert_eq!(request.chat_id, "@channel");
                assert_eq!(request.message, "We saw Alert 2024-05-01");
                assert_eq!(request.credentials.as_deref(), Some("tg-token"));
                request.tag
            }
            other => panic!("expected telegram send, got {:?}", other),
        };

        let cmd = flow.on_event(Event::StepCompleted {
            tag: telegram_tag,
            value: None,
        });

        let timer_tag = match cmd {
            Command::Do(Effect::StartTimer { duration, tag }) => {
                assert_eq!(duration.as_secs(), 600);
                tag
            }
            other => panic!("expected timer, got {:?}", other),
        };

        let cmd = flow.on_event(Event::TimerFired { tag: timer_tag });

        match cmd {
            Command::Do(Effect::FetchGoogleSheetCell(request)) => {
                assert_eq!(request.sheet_id, "sheet-123");
                assert_eq!(request.credentials.as_deref(), Some("docs-cred"));
            }
            other => panic!("expected new google sheet request, got {:?}", other),
        }
    }

    #[test]
    fn config_not_found_fails() {
        let mut flow = ReminderFlow::new();
        let tag = match flow.start() {
            Command::Do(Effect::LoadConfig { tag, .. }) => tag,
            other => panic!("unexpected command: {:?}", other),
        };

        let cmd = flow.on_event(Event::ConfigLoadFailed {
            tag,
            error: "missing".into(),
        });

        match cmd {
            Command::Done(Err(msg)) => assert!(msg.contains("failed to load configuration")),
            other => panic!("expected failure, got {:?}", other),
        }
    }

    #[test]
    fn invalid_yaml_fails() {
        let mut flow = ReminderFlow::new();
        let tag = match flow.start() {
            Command::Do(Effect::LoadConfig { tag, .. }) => tag,
            other => panic!("unexpected command: {:?}", other),
        };

        let cmd = flow.on_event(Event::ConfigLoaded {
            tag,
            path: "/tmp/remind.yaml".into(),
            contents: "invalid: [".into(),
        });

        match cmd {
            Command::Done(Err(msg)) => assert!(msg.contains("invalid configuration")),
            other => panic!("expected failure, got {:?}", other),
        }
    }

    #[test]
    fn missing_env_var_fails() {
        let mut flow = ReminderFlow::new();
        let tag = match flow.start() {
            Command::Do(Effect::LoadConfig { tag, .. }) => tag,
            other => panic!("unexpected command: {:?}", other),
        };

        let cmd = flow.on_event(Event::ConfigLoaded {
            tag,
            path: "cfg".into(),
            contents: config_yaml(),
        });

        let env_tag = match cmd {
            Command::Do(Effect::ReadEnvVar { name, tag }) => {
                assert_eq!(name, "GOOGLE_DOCS_TOKEN");
                tag
            }
            other => panic!("expected env read, got {:?}", other),
        };

        let cmd = flow.on_event(Event::EnvVarLoaded {
            tag: env_tag,
            name: "GOOGLE_DOCS_TOKEN".into(),
            value: None,
        });

        match cmd {
            Command::Done(Err(msg)) => {
                assert!(msg.contains("environment variable 'GOOGLE_DOCS_TOKEN' is required"))
            }
            other => panic!("expected failure, got {:?}", other),
        }
    }

    #[test]
    fn missing_variable_in_template_fails() {
        let yaml = r#"
run_every: "1m"
steps:
  - type: telegram
    chat_id: "@chat"
    message: "Hello {{missing}}"
"#;

        let mut flow = ReminderFlow::new();
        let tag = match flow.start() {
            Command::Do(Effect::LoadConfig { tag, .. }) => tag,
            other => panic!("unexpected command: {:?}", other),
        };

        let cmd = flow.on_event(Event::ConfigLoaded {
            tag,
            path: "cfg".into(),
            contents: yaml.to_string(),
        });

        match cmd {
            Command::Done(Err(msg)) => {
                assert!(msg.contains("value for 'missing' is not available"))
            }
            other => panic!("expected failure, got {:?}", other),
        }
    }
}
