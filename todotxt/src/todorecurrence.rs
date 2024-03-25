#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TodoRecurrence {
    Daily(u16, bool),
    BusinessDay(u16, bool),
    Weekly(u16, bool),
    Monthly(u16, bool),
    Yearly(u16, bool),
}
