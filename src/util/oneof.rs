use std::fmt::Debug;

#[derive(Debug, PartialEq, Eq)]
pub enum OneOf<T, O> {
    Left(T),
    Right(O),
}

impl<T, O> OneOf<T, O>
where
    T: Debug + PartialEq + Eq,
    O: Debug + PartialEq + Eq,
{
    pub fn peek_left(&self) -> Option<&T> {
        match self {
            Self::Left(v) => Some(v),
            _ => None,
        }
    }
    pub fn peek_left_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Left(v) => Some(v),
            _ => None,
        }
    }

    pub fn take_left(self) -> Option<T> {
        match self {
            Self::Left(v) => Some(v),
            _ => None,
        }
    }
    pub fn peek_right(&self) -> Option<&O> {
        match self {
            Self::Right(v) => Some(v),
            _ => None,
        }
    }

    pub fn peek_right_mut(&mut self) -> Option<&mut O> {
        match self {
            Self::Right(v) => Some(v),
            _ => None,
        }
    }
    pub fn take_right(self) -> Option<O> {
        match self {
            Self::Right(v) => Some(v),
            _ => None,
        }
    }
}
