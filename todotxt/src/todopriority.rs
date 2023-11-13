#[derive(Clone, Debug)]
pub struct  TodoPriority {
    pub priority: Option<char>,
}

impl TryFrom<String> for TodoPriority{
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        todo!()
    }
}
