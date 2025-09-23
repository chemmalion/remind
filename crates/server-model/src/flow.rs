use serde::de::value::MapAccessDeserializer;
use serde::de::{Error as DeError, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::convert::TryFrom;
use std::fmt;
use std::time::Duration;

pub const CONFIG_ENV_VAR: &str = "REMIND_CONFIG_PATH";
pub const DEFAULT_CONFIG_PATHS: [&str; 2] = ["~/.remind/config.yaml", "/etc/remind-config.yaml"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigDiscovery {
    pub env_var: &'static str,
    pub fallback_paths: &'static [&'static str],
}

pub const CONFIG_DISCOVERY: ConfigDiscovery = ConfigDiscovery {
    env_var: CONFIG_ENV_VAR,
    fallback_paths: &DEFAULT_CONFIG_PATHS,
};

#[derive(Debug, Clone)]
pub enum Effect {
    LoadConfig {
        discovery: ConfigDiscovery,
        tag: EffId,
    },
    ReadEnvVar {
        name: String,
        tag: EffId,
    },
    FetchGoogleSheetCell(GoogleSheetRequest),
    SearchEmails(EmailSearchRequest),
    SendTelegramMessage(TelegramRequest),
    StartTimer {
        duration: Duration,
        tag: EffId,
    },
}

#[derive(Debug, Clone)]
pub struct GoogleSheetRequest {
    pub sheet_id: String,
    pub worksheet: Option<String>,
    pub cell: CellRef,
    pub credentials: Option<String>,
    pub tag: EffId,
}

#[derive(Debug, Clone)]
pub struct EmailSearchRequest {
    pub account: String,
    pub field: EmailField,
    pub regex: String,
    pub credentials: Option<String>,
    pub tag: EffId,
}

#[derive(Debug, Clone)]
pub struct TelegramRequest {
    pub chat_id: String,
    pub message: String,
    pub credentials: Option<String>,
    pub tag: EffId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct CellRef {
    pub row: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmailField {
    Subject,
    Sender,
    Recipient,
}

#[derive(Debug, Clone)]
pub enum Event {
    ConfigLoaded {
        tag: EffId,
        path: String,
        contents: String,
    },
    ConfigLoadFailed {
        tag: EffId,
        error: String,
    },
    EnvVarLoaded {
        tag: EffId,
        name: String,
        value: Option<String>,
    },
    StepCompleted {
        tag: EffId,
        value: Option<String>,
    },
    StepFailed {
        tag: EffId,
        error: String,
    },
    TimerFired {
        tag: EffId,
    },
}

#[derive(Debug, Clone)]
pub enum Command {
    Do(Effect),
    Wait,
    Done(Result<(), String>),
}

pub struct ReminderFlow {
    stage: Stage,
    next_tag: u64,
    config: Option<FlowConfig>,
    env_values: HashMap<String, String>,
    resolved_credentials: HashMap<String, String>,
    pending_env: VecDeque<String>,
    current_env: Option<(String, EffId)>,
    run_state: Option<RunState>,
}

#[derive(Debug, Clone)]
enum Stage {
    Init,
    WaitingConfig { tag: EffId },
    LoadingEnv,
    Running,
    WaitingTimer { tag: EffId },
    Done(Result<(), String>),
}

#[derive(Debug, Clone)]
struct RunState {
    step_index: usize,
    variables: HashMap<String, String>,
    pending: Option<PendingStep>,
}

#[derive(Debug, Clone)]
struct PendingStep {
    step_index: usize,
    tag: EffId,
    store_as: Option<String>,
    require_value: bool,
}

#[derive(Debug, Clone)]
struct FlowConfig {
    run_every: Duration,
    credentials: BTreeMap<String, CredentialSource>,
    steps: Vec<Step>,
}

#[derive(Debug, Clone)]
enum CredentialSource {
    Value(String),
    EnvVar(String),
}

#[derive(Debug, Clone)]
enum Step {
    GoogleSheet(GoogleSheetStep),
    Email(EmailStep),
    Telegram(TelegramStep),
}

#[derive(Debug, Clone)]
struct GoogleSheetStep {
    sheet_id: ValueRef,
    worksheet: Option<ValueRef>,
    cell: CellRef,
    store_as: String,
    credentials: Option<String>,
}

#[derive(Debug, Clone)]
struct EmailStep {
    account: ValueRef,
    field: EmailField,
    regex: ValueRef,
    store_as: Option<String>,
    credentials: Option<String>,
}

#[derive(Debug, Clone)]
struct TelegramStep {
    chat_id: ValueRef,
    message: ValueRef,
    credentials: Option<String>,
}

#[derive(Debug, Clone)]
enum ValueRef {
    Literal(String),
    Env(String),
    Credential(String),
    Variable(String),
}

#[derive(Debug, Clone)]
enum FlowError {
    ConfigLoad(String),
    InvalidConfig(String),
    MissingEnvVar(String),
    MissingCredential(String),
    MissingVariable(String),
    InvalidTemplate(String),
    StepFailure { step_index: usize, message: String },
}

impl ReminderFlow {
    pub fn new() -> Self {
        Self {
            stage: Stage::Init,
            next_tag: 1,
            config: None,
            env_values: HashMap::new(),
            resolved_credentials: HashMap::new(),
            pending_env: VecDeque::new(),
            current_env: None,
            run_state: None,
        }
    }

    pub fn start(&mut self) -> Command {
        match self.stage {
            Stage::Init => {
                let tag = self.next_tag();
                self.stage = Stage::WaitingConfig { tag };
                Command::Do(Effect::LoadConfig {
                    discovery: CONFIG_DISCOVERY,
                    tag,
                })
            }
            Stage::Done(ref res) => Command::Done(res.clone()),
            _ => Command::Wait,
        }
    }

    pub fn on_event(&mut self, event: Event) -> Command {
        match &self.stage {
            Stage::WaitingConfig { tag } => match event {
                Event::ConfigLoaded {
                    tag: t, contents, ..
                } if *tag == t => match FlowConfig::from_yaml(&contents) {
                    Ok(cfg) => {
                        self.config = Some(cfg);
                        self.prepare_env_requests();
                        self.stage = Stage::LoadingEnv;
                        self.fetch_next_env_or_start_run()
                    }
                    Err(err) => self.finish_error(err),
                },
                Event::ConfigLoadFailed { tag: t, error } if *tag == t => {
                    self.finish_error(FlowError::ConfigLoad(error))
                }
                _ => Command::Wait,
            },
            Stage::LoadingEnv => match event {
                Event::EnvVarLoaded { tag, name, value } => {
                    if let Some((expected_name, expected_tag)) = self.current_env.clone() {
                        if expected_tag != tag || expected_name != name {
                            return Command::Wait;
                        }
                        self.current_env = None;
                        match value {
                            Some(v) => {
                                self.env_values.insert(name, v);
                                self.fetch_next_env_or_start_run()
                            }
                            None => self.finish_error(FlowError::MissingEnvVar(expected_name)),
                        }
                    } else {
                        Command::Wait
                    }
                }
                _ => Command::Wait,
            },
            Stage::Running => match event {
                Event::StepCompleted { tag, value } => {
                    if let Some(run_state) = self.run_state.as_mut() {
                        if let Some(pending) = run_state.pending.clone() {
                            if pending.tag != tag {
                                return Command::Wait;
                            }
                            run_state.pending = None;
                            if let Some(name) = pending.store_as {
                                match value {
                                    Some(val) => {
                                        run_state.variables.insert(name, val);
                                    }
                                    None => {
                                        return self.finish_error(FlowError::StepFailure {
                                            step_index: pending.step_index,
                                            message: "missing value in step result".into(),
                                        });
                                    }
                                }
                            } else if pending.require_value && value.is_none() {
                                return self.finish_error(FlowError::StepFailure {
                                    step_index: pending.step_index,
                                    message: "missing value in step result".into(),
                                });
                            }
                            run_state.step_index += 1;
                        } else {
                            return Command::Wait;
                        }
                    } else {
                        return Command::Wait;
                    }
                    self.advance_run()
                }
                Event::StepFailed { tag, error } => {
                    if let Some(run) = &self.run_state {
                        if let Some(pending) = &run.pending {
                            if pending.tag == tag {
                                return self.finish_error(FlowError::StepFailure {
                                    step_index: pending.step_index,
                                    message: error,
                                });
                            }
                        }
                    }
                    Command::Wait
                }
                Event::TimerFired { .. } => Command::Wait,
                Event::ConfigLoaded { .. }
                | Event::ConfigLoadFailed { .. }
                | Event::EnvVarLoaded { .. } => Command::Wait,
            },
            Stage::WaitingTimer { tag } => match event {
                Event::TimerFired { tag: t } if *tag == t => self.start_run(),
                _ => Command::Wait,
            },
            Stage::Init => Command::Wait,
            Stage::Done(ref res) => Command::Done(res.clone()),
        }
    }

    pub fn done(&self) -> Result<(), String> {
        match &self.stage {
            Stage::Done(res) => res.clone(),
            _ => Ok(()),
        }
    }

    fn next_tag(&mut self) -> EffId {
        let id = EffId(self.next_tag);
        self.next_tag += 1;
        id
    }

    fn prepare_env_requests(&mut self) {
        self.pending_env.clear();
        self.env_values.clear();
        self.resolved_credentials.clear();
        self.current_env = None;
        if let Some(cfg) = &self.config {
            let requests: BTreeSet<String> = cfg.env_requests();
            self.pending_env = requests.into_iter().collect();
        }
    }

    fn fetch_next_env_or_start_run(&mut self) -> Command {
        if self.current_env.is_some() {
            return Command::Wait;
        }
        if let Some(name) = self.pending_env.pop_front() {
            let tag = self.next_tag();
            self.current_env = Some((name.clone(), tag));
            Command::Do(Effect::ReadEnvVar { name, tag })
        } else {
            match self.finalize_credentials() {
                Ok(()) => self.start_run(),
                Err(err) => self.finish_error(err),
            }
        }
    }

    fn finalize_credentials(&mut self) -> Result<(), FlowError> {
        let cfg = match &self.config {
            Some(cfg) => cfg,
            None => return Ok(()),
        };
        let mut resolved = HashMap::new();
        for (name, source) in &cfg.credentials {
            let value = match source {
                CredentialSource::Value(val) => val.clone(),
                CredentialSource::EnvVar(env) => self
                    .env_values
                    .get(env)
                    .cloned()
                    .ok_or_else(|| FlowError::MissingEnvVar(env.clone()))?,
            };
            resolved.insert(name.clone(), value);
        }
        self.resolved_credentials = resolved;
        Ok(())
    }

    fn start_run(&mut self) -> Command {
        if self.config.is_none() {
            return Command::Wait;
        }
        self.run_state = Some(RunState {
            step_index: 0,
            variables: HashMap::new(),
            pending: None,
        });
        self.stage = Stage::Running;
        self.advance_run()
    }

    fn advance_run(&mut self) -> Command {
        let (run_every, steps_len) = match &self.config {
            Some(cfg) => (cfg.run_every, cfg.steps.len()),
            None => return Command::Wait,
        };
        let step_index = {
            let run = match &mut self.run_state {
                Some(run) => run,
                None => return Command::Wait,
            };
            if run.pending.is_some() {
                return Command::Wait;
            }
            if run.step_index >= steps_len {
                self.run_state = None;
                let tag = self.next_tag();
                self.stage = Stage::WaitingTimer { tag };
                return Command::Do(Effect::StartTimer {
                    duration: run_every,
                    tag,
                });
            }
            run.step_index
        };
        match self.execute_step(step_index) {
            Ok(cmd) => cmd,
            Err(err) => self.finish_error(err),
        }
    }

    fn execute_step(&mut self, step_index: usize) -> Result<Command, FlowError> {
        let cfg = self
            .config
            .as_ref()
            .ok_or_else(|| FlowError::InvalidConfig("configuration missing".into()))?;
        let step = cfg
            .steps
            .get(step_index)
            .cloned()
            .ok_or_else(|| FlowError::InvalidConfig(format!("step {step_index} is missing")))?;
        let variables_snapshot = self
            .run_state
            .as_ref()
            .map(|run| run.variables.clone())
            .ok_or_else(|| FlowError::InvalidConfig("run state missing".into()))?;

        let (pending, effect) = match step {
            Step::GoogleSheet(step) => {
                let GoogleSheetStep {
                    sheet_id,
                    worksheet,
                    cell,
                    store_as,
                    credentials,
                } = step;
                let sheet_id = self.resolve_value(&sheet_id, &variables_snapshot)?;
                let worksheet = match worksheet {
                    Some(value) => Some(self.resolve_value(&value, &variables_snapshot)?),
                    None => None,
                };
                let credentials = match credentials {
                    Some(name) => Some(self.credential_value(&name)?),
                    None => None,
                };
                let tag = self.next_tag();
                let pending = PendingStep {
                    step_index,
                    tag,
                    store_as: Some(store_as),
                    require_value: true,
                };
                let effect = Effect::FetchGoogleSheetCell(GoogleSheetRequest {
                    sheet_id,
                    worksheet,
                    cell,
                    credentials,
                    tag,
                });
                (pending, effect)
            }
            Step::Email(step) => {
                let EmailStep {
                    account,
                    field,
                    regex,
                    store_as,
                    credentials,
                } = step;
                let account = self.resolve_value(&account, &variables_snapshot)?;
                let regex = self.resolve_value(&regex, &variables_snapshot)?;
                let credentials = match credentials {
                    Some(name) => Some(self.credential_value(&name)?),
                    None => None,
                };
                let tag = self.next_tag();
                let pending = PendingStep {
                    step_index,
                    tag,
                    store_as: store_as.clone(),
                    require_value: store_as.is_some(),
                };
                let effect = Effect::SearchEmails(EmailSearchRequest {
                    account,
                    field,
                    regex,
                    credentials,
                    tag,
                });
                (pending, effect)
            }
            Step::Telegram(step) => {
                let TelegramStep {
                    chat_id,
                    message,
                    credentials,
                } = step;
                let chat_id = self.resolve_value(&chat_id, &variables_snapshot)?;
                let message = self.resolve_value(&message, &variables_snapshot)?;
                let credentials = match credentials {
                    Some(name) => Some(self.credential_value(&name)?),
                    None => None,
                };
                let tag = self.next_tag();
                let pending = PendingStep {
                    step_index,
                    tag,
                    store_as: None,
                    require_value: false,
                };
                let effect = Effect::SendTelegramMessage(TelegramRequest {
                    chat_id,
                    message,
                    credentials,
                    tag,
                });
                (pending, effect)
            }
        };

        if let Some(run) = self.run_state.as_mut() {
            run.pending = Some(pending);
        }
        Ok(Command::Do(effect))
    }

    fn credential_value(&self, name: &str) -> Result<String, FlowError> {
        self.resolved_credentials
            .get(name)
            .cloned()
            .ok_or_else(|| FlowError::MissingCredential(name.to_string()))
    }

    fn resolve_value(
        &self,
        value: &ValueRef,
        variables: &HashMap<String, String>,
    ) -> Result<String, FlowError> {
        match value {
            ValueRef::Literal(template) => self.render_template(template, variables),
            ValueRef::Env(name) => self
                .env_values
                .get(name)
                .cloned()
                .ok_or_else(|| FlowError::MissingEnvVar(name.clone())),
            ValueRef::Credential(name) => self.credential_value(name),
            ValueRef::Variable(name) => variables
                .get(name)
                .cloned()
                .ok_or_else(|| FlowError::MissingVariable(name.clone())),
        }
    }

    fn render_template(
        &self,
        template: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String, FlowError> {
        let mut output = String::new();
        let mut remainder = template;
        while let Some(start) = remainder.find("{{") {
            let (before, after_start) = remainder.split_at(start);
            output.push_str(before);
            let after_brace = &after_start[2..];
            if let Some(end) = after_brace.find("}}") {
                let placeholder = after_brace[..end].trim();
                if placeholder.is_empty() {
                    return Err(FlowError::InvalidTemplate(
                        "empty placeholder in template".into(),
                    ));
                }
                let replacement = variables
                    .get(placeholder)
                    .or_else(|| self.resolved_credentials.get(placeholder))
                    .or_else(|| self.env_values.get(placeholder))
                    .cloned()
                    .ok_or_else(|| FlowError::MissingVariable(placeholder.to_string()))?;
                output.push_str(&replacement);
                remainder = &after_brace[end + 2..];
            } else {
                return Err(FlowError::InvalidTemplate(
                    "unclosed placeholder in template".into(),
                ));
            }
        }
        output.push_str(remainder);
        Ok(output)
    }

    fn finish_error(&mut self, err: FlowError) -> Command {
        let msg = err.to_string();
        self.stage = Stage::Done(Err(msg.clone()));
        self.run_state = None;
        Command::Done(Err(msg))
    }
}

impl FlowConfig {
    fn from_yaml(contents: &str) -> Result<Self, FlowError> {
        let raw: RawConfig = serde_yaml::from_str(contents)
            .map_err(|err| FlowError::InvalidConfig(err.to_string()))?;
        let run_every = parse_duration(&raw.run_every)
            .ok_or_else(|| FlowError::InvalidConfig("invalid run_every value".into()))?;
        let mut credentials = BTreeMap::new();
        for (name, cred) in raw.credentials {
            let source = CredentialSource::from_map(cred)
                .map_err(|err| FlowError::InvalidConfig(format!("credential '{name}': {err}")))?;
            credentials.insert(name, source);
        }
        let mut steps = Vec::new();
        for step in raw.steps {
            steps.push(step.try_into()?);
        }
        Ok(Self {
            run_every,
            credentials,
            steps,
        })
    }

    fn env_requests(&self) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        for source in self.credentials.values() {
            if let CredentialSource::EnvVar(name) = source {
                set.insert(name.clone());
            }
        }
        for step in &self.steps {
            step.collect_env(&mut set);
        }
        set
    }
}

impl Step {
    fn collect_env(&self, set: &mut BTreeSet<String>) {
        match self {
            Step::GoogleSheet(step) => {
                step.sheet_id.collect_env(set);
                if let Some(ws) = &step.worksheet {
                    ws.collect_env(set);
                }
            }
            Step::Email(step) => {
                step.account.collect_env(set);
                step.regex.collect_env(set);
            }
            Step::Telegram(step) => {
                step.chat_id.collect_env(set);
                step.message.collect_env(set);
            }
        }
    }
}

impl ValueRef {
    fn collect_env(&self, set: &mut BTreeSet<String>) {
        if let ValueRef::Env(name) = self {
            set.insert(name.clone());
        }
    }
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowError::ConfigLoad(err) => write!(f, "failed to load configuration: {err}"),
            FlowError::InvalidConfig(err) => write!(f, "invalid configuration: {err}"),
            FlowError::MissingEnvVar(name) => {
                write!(f, "environment variable '{name}' is required")
            }
            FlowError::MissingCredential(name) => {
                write!(f, "credential '{name}' is not defined")
            }
            FlowError::MissingVariable(name) => {
                write!(f, "value for '{name}' is not available")
            }
            FlowError::InvalidTemplate(err) => write!(f, "template error: {err}"),
            FlowError::StepFailure {
                step_index,
                message,
            } => {
                write!(f, "step {} failed: {message}", step_index + 1)
            }
        }
    }
}

impl CredentialSource {
    fn from_map(map: ValueMap) -> Result<Self, String> {
        if map.credential.is_some() || map.var.is_some() {
            return Err("credential may only specify 'value' or 'env'".into());
        }
        match (map.value, map.env) {
            (Some(value), None) => Ok(CredentialSource::Value(value)),
            (None, Some(env)) => Ok(CredentialSource::EnvVar(env)),
            (Some(_), Some(_)) => Err("credential must specify either 'value' or 'env'".into()),
            (None, None) => Err("credential must specify 'value' or 'env'".into()),
        }
    }
}

impl ValueRef {
    fn from_map(map: ValueMap) -> Result<Self, String> {
        match (map.value, map.env, map.credential, map.var) {
            (Some(value), None, None, None) => Ok(ValueRef::Literal(value)),
            (None, Some(env), None, None) => Ok(ValueRef::Env(env)),
            (None, None, Some(cred), None) => Ok(ValueRef::Credential(cred)),
            (None, None, None, Some(var)) => Ok(ValueRef::Variable(var)),
            (None, None, None, None) => Err(
                "value reference must specify one of 'value', 'env', 'credential', or 'var'".into(),
            ),
            _ => Err("value reference must specify only one source".into()),
        }
    }
}

impl<'de> Deserialize<'de> for ValueRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueRefVisitor;

        impl<'de> Visitor<'de> for ValueRefVisitor {
            type Value = ValueRef;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or a value reference map")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(ValueRef::Literal(v.to_string()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(ValueRef::Literal(v))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let value_map = ValueMap::deserialize(MapAccessDeserializer::new(map))?;
                ValueRef::from_map(value_map).map_err(A::Error::custom)
            }
        }

        deserializer.deserialize_any(ValueRefVisitor)
    }
}

impl<'de> Deserialize<'de> for CredentialSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CredentialVisitor;

        impl<'de> Visitor<'de> for CredentialVisitor {
            type Value = CredentialSource;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a credential definition")
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let value_map = ValueMap::deserialize(MapAccessDeserializer::new(map))?;
                CredentialSource::from_map(value_map).map_err(A::Error::custom)
            }
        }

        deserializer.deserialize_any(CredentialVisitor)
    }
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    run_every: String,
    #[serde(default)]
    credentials: BTreeMap<String, ValueMap>,
    steps: Vec<RawStep>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawStep {
    GoogleSheet {
        sheet_id: ValueRef,
        #[serde(default)]
        worksheet: Option<ValueRef>,
        cell: CellRef,
        store_as: String,
        #[serde(default)]
        credentials: Option<String>,
    },
    Email {
        account: ValueRef,
        field: EmailField,
        regex: ValueRef,
        #[serde(default)]
        store_as: Option<String>,
        #[serde(default)]
        credentials: Option<String>,
    },
    Telegram {
        chat_id: ValueRef,
        message: ValueRef,
        #[serde(default)]
        credentials: Option<String>,
    },
}

impl TryFrom<RawStep> for Step {
    type Error = FlowError;

    fn try_from(value: RawStep) -> Result<Self, Self::Error> {
        match value {
            RawStep::GoogleSheet {
                sheet_id,
                worksheet,
                cell,
                store_as,
                credentials,
            } => Ok(Step::GoogleSheet(GoogleSheetStep {
                sheet_id,
                worksheet,
                cell,
                store_as,
                credentials,
            })),
            RawStep::Email {
                account,
                field,
                regex,
                store_as,
                credentials,
            } => Ok(Step::Email(EmailStep {
                account,
                field,
                regex,
                store_as,
                credentials,
            })),
            RawStep::Telegram {
                chat_id,
                message,
                credentials,
            } => Ok(Step::Telegram(TelegramStep {
                chat_id,
                message,
                credentials,
            })),
        }
    }
}

#[derive(Debug)]
struct ValueMap {
    value: Option<String>,
    env: Option<String>,
    credential: Option<String>,
    var: Option<String>,
}

impl<'de> Deserialize<'de> for ValueMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueMapVisitor;

        impl<'de> Visitor<'de> for ValueMapVisitor {
            type Value = ValueMap;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a value map")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut value = None;
                let mut env = None;
                let mut credential = None;
                let mut var = None;
                while let Some(key) = access.next_key::<String>()? {
                    match key.as_str() {
                        "value" => {
                            if value.is_some() {
                                return Err(A::Error::custom("duplicate 'value' field"));
                            }
                            value = Some(access.next_value()?);
                        }
                        "env" => {
                            if env.is_some() {
                                return Err(A::Error::custom("duplicate 'env' field"));
                            }
                            env = Some(access.next_value()?);
                        }
                        "credential" => {
                            if credential.is_some() {
                                return Err(A::Error::custom("duplicate 'credential' field"));
                            }
                            credential = Some(access.next_value()?);
                        }
                        "var" => {
                            if var.is_some() {
                                return Err(A::Error::custom("duplicate 'var' field"));
                            }
                            var = Some(access.next_value()?);
                        }
                        other => {
                            return Err(A::Error::unknown_field(
                                other,
                                &["value", "env", "credential", "var"],
                            ));
                        }
                    }
                }
                Ok(ValueMap {
                    value,
                    env,
                    credential,
                    var,
                })
            }
        }

        deserializer.deserialize_map(ValueMapVisitor)
    }
}

fn parse_duration(input: &str) -> Option<Duration> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut digits = String::new();
    let mut unit = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() {
            if !unit.is_empty() {
                return None;
            }
            digits.push(ch);
        } else {
            unit.push(ch);
        }
    }
    let value: u64 = digits.parse().ok()?;
    if unit.is_empty() {
        return Some(Duration::from_secs(value));
    }
    match unit.as_str() {
        "s" => Some(Duration::from_secs(value)),
        "m" => Some(Duration::from_secs(value * 60)),
        "h" => Some(Duration::from_secs(value * 3600)),
        "d" => Some(Duration::from_secs(value * 86400)),
        _ => None,
    }
}
