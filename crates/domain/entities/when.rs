pub enum When {
    OncePer(Duration),
    EachDay { at: Time, until: Time },
}
