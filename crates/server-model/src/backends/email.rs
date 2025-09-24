use common::error::AResult;

pub trait EmailBackend {
    fn list_emails(creds: &Creds, limits: &Limits) -> AResult<Vec<Email>>;
}

pub struct Creds {
    // todo: put the fields actually usually needed for creds for email client
}

pub struct Limits {
    // The last email read from the previous run. If it is set it means the only newer
    // emails should be taken. So that if that email is found -- the gathering is done,
    // because that email is an old and olready was received from previous runs.
    pub last_prev_email: Option<Email>,
    // If it is set then the gathering is stopped when that date is reached.
    // So that it limit how many old emails can be taken.
    pub date: Option<DateTime>,
}
