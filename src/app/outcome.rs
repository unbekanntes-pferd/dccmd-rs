#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeMessageKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeMessage {
    pub kind: OutcomeMessageKind,
    pub text: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    pub messages: Vec<OutcomeMessage>,
}

impl CommandOutcome {
    pub fn push(&mut self, kind: OutcomeMessageKind, text: impl Into<String>) {
        self.messages.push(OutcomeMessage {
            kind,
            text: text.into(),
        });
    }
}
