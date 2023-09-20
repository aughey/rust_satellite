//! Common utilities that are shared across crates.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

use std::str::FromStr;

/// A string that can be either a String or a &str
/// This is used to optimize for values that could be either.
/// Most of the time these will be references and we don't need
/// to allocate anything, but if we need to, we can.
#[derive(Debug, Clone)]
pub enum StringOrStr<'a> {
    /// A string that owns its data
    String(String),
    /// A string reference
    Str(&'a str),
}
impl std::hash::Hash for StringOrStr<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}
/// Convert from a string reference
impl<'a> From<&'a str> for StringOrStr<'a> {
    fn from(s: &'a str) -> Self {
        Self::Str(s)
    }
}
/// Convert from a string
impl From<String> for StringOrStr<'_> {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}
/// Get the underlying string reference
impl AsRef<str> for StringOrStr<'_> {
    fn as_ref(&self) -> &str {
        match self {
            Self::String(s) => s.as_ref(),
            Self::Str(s) => s,
        }
    }
}
/// Methods that behave like string things
impl StringOrStr<'_> {
    /// Length of internal string
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }

    /// Is the internal string empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the internal string as a string reference.
    /// This is the same as as_ref(), but uses the as_str
    /// convention from the standard library.
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    /// Parse into a type that implements FromStr
    pub fn parse<T>(&self) -> Result<T, T::Err>
    where
        T: FromStr,
    {
        self.as_ref().parse()
    }
}

/// PartialEq compares the references because we
/// care if the string value inside whatever enum
/// it is are the same
/// ```
/// # use common::StringOrStr;
/// assert_eq!(StringOrStr::Str("John"), StringOrStr::String("John".to_string()));
/// ```
impl PartialEq for StringOrStr<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl Eq for StringOrStr<'_> {}
