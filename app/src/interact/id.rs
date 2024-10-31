#[derive(Debug, Clone, Hash, Ord, PartialOrd, PartialEq, Eq, Copy)]
pub enum InteractID<ID> {
    Scope(ID),
    Command(ID),
}

pub const COMMAND_MASK: u8 = 0b0000_1111;
pub const SCOPE_MASK: u8 = 0b1111_0000;

pub const PROMPT_ID: InteractID<u8> = InteractID::Command(0b0);
pub const PUSH_ID: InteractID<u8> = InteractID::Command(0b1);
pub const RAG_PUSH_ID: InteractID<u8> = InteractID::Command(0b10);

pub const GLOBAL_ID: InteractID<u8> = InteractID::Scope(0b0);
pub const DOCUMENT_ID: InteractID<u8> = InteractID::Scope(0b0001_0000);

pub const DOCUMENT_CHARACTER: InteractID<char> = InteractID::Scope('^');
pub const GLOBAL_CHARACTER: InteractID<char> = InteractID::Scope('_');

pub const PROMPT_CHARACTER: InteractID<char> = InteractID::Command('@');
pub const PUSH_CHARACTER: InteractID<char> = InteractID::Command('+');
pub const RAG_PUSH_CHARACTER: InteractID<char> = InteractID::Command('$');

impl<ID> AsRef<ID> for InteractID<ID> {
    fn as_ref(&self) -> &ID {
        match self {
            Self::Command(id) => &id,
            Self::Scope(id) => &id,
        }
    }
}

pub fn u8_to_binary_string(num: u8) -> String {
    let mut result = String::with_capacity(8);
    for i in (0..8).rev() {
        if num & (1 << i) != 0 {
            result.push('1');
        } else {
            result.push('0');
        }
    }
    result
}

pub fn human_readable_int(int: u8) -> String {
    let command_masked = int & COMMAND_MASK;
    let command_str = match command_masked {
        _ if command_masked == *PUSH_ID.as_ref() => "PUSH",
        _ if command_masked == *PROMPT_ID.as_ref() => "PROMPT",
        _ if command_masked == *RAG_PUSH_ID.as_ref() => "RAG_PUSH",
        other => &format!("UNKNOWN COMMAND ID: {}", u8_to_binary_string(other)),
    };

    let scope_masked = int & SCOPE_MASK;
    let scope_str = match scope_masked {
        _ if scope_masked == *GLOBAL_ID.as_ref() => "GLOBAL",
        _ if scope_masked == *DOCUMENT_ID.as_ref() => "DOCUMENT",
        other => &format!("UNKNOWN SCOPE ID: {}", u8_to_binary_string(other)),
    };

    format!("{command_str}_{scope_str}")
}
