pub struct TahtiLasku(u16);

pub const fn find(email: &Email) -> Option<FoundTahtiLasku> {
    todo!()
}

pub struct FoundTahtiLasku {
    pub lasku: TahtiLasku,
    pub invoice: DateTime,
}

pub const fn remind(about: &FoundTahtiLasku) -> When {
    When::EachDay {
        at: todo!(),
        until: todo!(),
    }
}

pub const fn get_sheet() -> sheet::Path {
    sheet::Path::TahtiLasku
}

pub const fn check_payment(
    l: &FoundTahtiLasku,
    s: &Sheet,
) -> Result<Option<PaidTahtiLasku>, SheetAnalysisError> {
    todo!()
}

pub enum SheetAnalysisError {}

pub struct PaidTahtiLasku {
    pub lasku: Lasku,
    pub invoice: DateTime,
    pub paid: DateTime,
}
