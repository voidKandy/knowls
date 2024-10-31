use super::{
    error::{InteractError, InteractResult},
    id::*,
};
use std::{collections::HashMap, fmt::Debug};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct InteractRegistry {
    char_lookup: HashMap<InteractID<char>, InteractID<u8>>,
    id_lookup: HashMap<InteractID<u8>, InteractID<char>>,
}

impl Default for InteractRegistry {
    fn default() -> Self {
        let mut registered = Self::new();
        registered.insert(GLOBAL_CHARACTER, GLOBAL_ID);
        registered.insert(DOCUMENT_CHARACTER, DOCUMENT_ID);

        registered.insert(PUSH_CHARACTER, PUSH_ID);
        registered.insert(RAG_PUSH_CHARACTER, RAG_PUSH_ID);
        registered.insert(PROMPT_CHARACTER, PROMPT_ID);
        registered
    }
}

impl InteractRegistry {
    fn new() -> Self {
        Self {
            char_lookup: HashMap::new(),
            id_lookup: HashMap::new(),
        }
    }

    fn insert(&mut self, char: InteractID<char>, id: InteractID<u8>) {
        self.char_lookup.insert(char.clone(), id);
        self.id_lookup.insert(id, char);

        if self.char_lookup.len() != self.id_lookup.len() {
            warn!("Lookup tables out of sync: {self:#?}");
            for (char, id) in self.char_lookup.iter() {
                if self.id_lookup.get(&id) != Some(&char) {
                    warn!("mismatch\nchar lookup: {char:#?}\nid: {id:#?}")
                }
            }
            panic!("lookup tables should be synced")
        }
    }

    /// Interact integer should be pre masked
    pub fn get_interact_char(&self, interact: InteractID<u8>) -> Option<&InteractID<char>> {
        self.id_lookup.get(&interact)
    }

    pub fn get_interact_integer(&self, interact: InteractID<char>) -> Option<&InteractID<u8>> {
        self.char_lookup.get(&interact)
    }

    pub fn register_scope(&mut self, char: &char) -> InteractResult<()> {
        let max = self.max_scope_id();
        warn!("registering scope for char: {char}\ncurrent max: {max}");

        let id = Self::increment_masked_value(max, SCOPE_MASK)?;
        warn!("id: {id}");

        if id == SCOPE_MASK {
            return Err(InteractError::RegistryFull);
        }

        self.insert(InteractID::Scope(*char), InteractID::Scope(id));
        Ok(())
    }

    fn all_registered_scope_ids(&self) -> Vec<u8> {
        self.char_lookup
            .iter()
            .filter_map(|(char, id)| {
                if let InteractID::<char>::Scope(_) = char {
                    Some(*id.as_ref())
                } else {
                    None
                }
            })
            .collect()
    }

    fn all_registered_command_ids(&self) -> Vec<u8> {
        self.char_lookup
            .iter()
            .filter_map(|(char, id)| {
                if let InteractID::<char>::Command(_) = char {
                    Some(*id.as_ref())
                } else {
                    None
                }
            })
            .collect()
    }

    fn increment_masked_value(value: u8, mask: u8) -> InteractResult<u8> {
        if value == mask {
            return Err(InteractError::RegistryFull);
        }
        let masked_value = value & mask;

        let shift_amount = mask.trailing_zeros();
        let incremented_value = ((masked_value >> shift_amount) + 1) << shift_amount;

        let new_value = (value & !mask) | (incremented_value & mask);

        if new_value > mask {
            panic!("overflow when incrementing masked value");
        }

        Ok(new_value)
    }

    fn max_scope_id(&self) -> u8 {
        *self
            .id_lookup
            .keys()
            .collect::<Vec<&InteractID<u8>>>()
            .iter_mut()
            .map(|id| {
                let mut val: u8 = *id.as_ref();
                if let InteractID::<u8>::Scope(_) = *id {
                    val <<= 4;
                }
                val
            })
            .collect::<Vec<u8>>()
            .iter()
            .max()
            .expect("no shot this fails")
    }

    pub fn try_get_interact(&self, string: &String) -> Option<u8> {
        let first_non_whitespace_pos = string.chars().position(|c| !c.is_whitespace())?;

        let command_char = string.chars().nth(first_non_whitespace_pos)?;
        let command_id = *self
            .char_lookup
            .keys()
            .find(|ch| InteractID::<char>::Command(command_char) == **ch)
            .and_then(|ic| self.char_lookup.get(ic))?;

        let scope_char = string.chars().nth(first_non_whitespace_pos + 1)?;
        let scope_id = *self
            .char_lookup
            .keys()
            .find(|ch| InteractID::<char>::Scope(scope_char) == **ch)
            .and_then(|ic| self.char_lookup.get(ic))?;

        Some(command_id.as_ref() + scope_id.as_ref())
    }

    /// Given a byte, returns the associated command and scope ids.
    /// Will error if either the command or scope id are not within the registry
    pub fn interract_tuple(&self, id: u8) -> InteractResult<(InteractID<u8>, InteractID<u8>)> {
        let command = id & COMMAND_MASK;
        let scope = id & SCOPE_MASK;

        if !self.all_registered_command_ids().contains(&command)
            || !self.all_registered_scope_ids().contains(&scope)
        {
            return Err(InteractError::InvalidInteractId(id));
        }

        let command = InteractID::<u8>::Command(command);
        let scope = InteractID::<u8>::Scope(scope);

        return Ok((command, scope));
    }
}

mod tests {
    
    

    #[test]
    fn incrementation_works() {
        let val = 0b0001_0000;
        let inc = InteractRegistry::increment_masked_value(val, SCOPE_MASK).unwrap();
        assert_eq!(inc, 0b0010_0000);

        let val = 0b0010_0000;
        let inc = InteractRegistry::increment_masked_value(val, SCOPE_MASK).unwrap();
        assert_eq!(inc, 0b0011_0000);

        let val = 0b0011_0000;
        let inc = InteractRegistry::increment_masked_value(val, SCOPE_MASK).unwrap();
        assert_eq!(inc, 0b0100_0000);

        let val = 0b0110_0000;
        let inc = InteractRegistry::increment_masked_value(val, SCOPE_MASK).unwrap();
        assert_eq!(inc, 0b0111_0000);

        let inc = InteractRegistry::increment_masked_value(SCOPE_MASK, SCOPE_MASK);
        assert!(inc.is_err());
    }
}
