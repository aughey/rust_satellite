use std::collections::HashMap;

pub use anyhow::Result;
use nom::{
    branch::alt,
    bytes::complete::{tag, take},
    character::complete::multispace0,
    Finish, IResult,
};

pub struct ParseMap<'a> {
    pub(crate) map: HashMap<String, StringOrStr<'a>>,
}

impl<'a> ParseMap<'a> {
    pub(crate) fn get(&self, key: &str) -> Result<StringOrStr<'a>> {
        if let Some(value) = self.map.get(key) {
            Ok(value.clone())
        } else {
            Err(anyhow::anyhow!("Key {} not found", key))
        }
    }

    #[cfg(test)]
    pub(crate) fn keys(&self) -> std::collections::hash_map::Keys<String, StringOrStr> {
        self.map.keys()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
}

impl<'a> TryFrom<&'a str> for ParseMap<'a> {
    type Error = nom::error::Error<&'a str>;

    fn try_from(value: &'a str) -> std::result::Result<Self, Self::Error> {
        Ok(str_to_key_value(value).finish()?.1)
    }
}

// returns the next character, or the subsequent characters if the first is a backslash
fn char_or_escaped_char(data: &str) -> IResult<&str, &str> {
    let (data, maybe_backslash) = take(1usize)(data)?;

    if data == "\\" {
        let (data, escaped_char) = take(1usize)(data)?;
        Ok((data, escaped_char))
    } else {
        Ok((data, maybe_backslash))
    }
}

// parse a quoted string, with escaped characters
fn quoted_string(data: &str) -> IResult<&str, StringOrStr> {
    // initial quote
    let (data, _) = tag("\"")(data)?;
    // char_or_escaped_char will return the next value.  Accumulate this until
    let mut head = data;
    let mut accum = String::new();
    loop {
        let (data, value) = char_or_escaped_char(head)?;
        head = data;
        if value == "\"" {
            break;
        }
        accum.push_str(value);
    }

    Ok((head, StringOrStr::String(accum)))
}

fn unquoted_string(data: &str) -> IResult<&str, StringOrStr> {
    let (data, value) = nom::bytes::complete::take_while(|c: char| !c.is_whitespace())(data)?;
    Ok((data, StringOrStr::Str(value)))
}

pub(crate) fn str_to_key_value(data: &str) -> IResult<&str, ParseMap> {
    let mut key_values = HashMap::new();

    let mut head = data;
    loop {
        // Check for empty
        if head.is_empty() {
            break;
        }
        // using nom, trim whitesapce
        let (data, _) = multispace0(head)?;
        // Check again just in case trailing whitespace
        if data.is_empty() {
            head = data;
            break;
        }
        // parse key, letters, numbers, underscores, dashes
        let (data, key) = nom::bytes::complete::take_while(|c: char| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-'
        })(data)?;
        // parse =
        let (data, _) = tag("=")(data)?;
        // parse value, a quoted string or a non-quoted string with no whitespace
        let (data, value) = alt((quoted_string, unquoted_string))(data)?;
        // insert into map
        key_values.insert(key.to_string(), value);
        head = data;
    }

    Ok((head, ParseMap { map: key_values }))
}

#[derive(Debug, Clone)]
pub enum StringOrStr<'a> {
    String(String),
    Str(&'a str),
}
impl From<&str> for StringOrStr<'_> {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}
impl<'a> AsRef<str> for StringOrStr<'a> {
    fn as_ref(&self) -> &str {
        match self {
            Self::String(s) => s.as_ref(),
            Self::Str(s) => s,
        }
    }
}
impl StringOrStr<'_> {
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }
}
impl PartialEq for StringOrStr<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl Eq for StringOrStr<'_> {}
