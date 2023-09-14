use std::{collections::HashMap, str::FromStr};

pub use anyhow::Result;
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while},
    character::complete::multispace0,
    Finish, IResult,
};

pub struct ParseMap<'a> {
    map: HashMap<&'a str, StringOrStr<'a>>,
}

impl<'a> ParseMap<'a> {
    pub fn get(&self, key: &str) -> Result<StringOrStr<'a>> {
        self.map
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Key {} not found", key))
    }

    #[cfg(test)]
    fn keys(&self) -> std::collections::hash_map::Keys<&str, StringOrStr> {
        self.map.keys()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }
}

impl<'a> TryFrom<&'a str> for ParseMap<'a> {
    type Error = nom::error::Error<&'a str>;

    fn try_from(value: &'a str) -> std::result::Result<Self, Self::Error> {
        Ok(str_to_key_value(value).finish()?.1)
    }
}

// parse a quoted string, with escaped characters
fn quoted_string(data: &str) -> IResult<&str, StringOrStr> {
    // initial quote
    let (data, _) = tag("\"")(data)?;

    // An optional string to accumulate into.  Don't allocate a string
    // if we don't have to.  We have to if we have a backslash to escape.
    let mut accum: Option<String> = None;

    // the head of our data, we'll move this forward as we parse
    let mut head = data;
    loop {
        // Move forward until we find a backslash or a quote
        let (data, value) = take_while(|c: char| c != '\\' && c != '"')(head)?;

        // look at the next char, if it's a quote, we're done, it was consumed so return
        let (data, maybe_quote) = take(1usize)(data)?;
        if maybe_quote == "\"" {
            // if we've accumulated strings so far, add this to the end and return,
            // otherwise we return the string reference and don't allocate.
            return match accum {
                Some(accum) => Ok((data, (accum + value).into())),
                None => Ok((data, value.into())),
            };
        }
        // Crap, there's a backslash

        // create an accumulator if we haven't already and append the string we've parsed so far
        let to_append = accum.get_or_insert_with(|| String::new());
        to_append.push_str(value);

        // since we have a backslash, we need to parse the next character and append that too
        let (data, escaped_char) = take(1usize)(data)?;
        to_append.push_str(escaped_char);

        // Move the head forward and look for the next one.
        head = data;
    }
}

fn unquoted_string(data: &str) -> IResult<&str, StringOrStr> {
    let (data, value) = take_while(|c: char| !c.is_whitespace())(data)?;
    Ok((data, value.into()))
}

fn str_to_key_value(data: &str) -> IResult<&str, ParseMap> {
    let mut key_values = HashMap::new();

    let mut head = data;
    while !head.is_empty() {
        // using nom, trim whitesapce
        let (data, _) = multispace0(head)?;

        // Check again just in case leading whitespace
        if data.is_empty() {
            head = data;
            break;
        }

        // parse key, letters, numbers, underscores, dashes
        let (data, key) =
            take_while(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-')(data)?;

        let (data, _) = multispace0(data)?;
        // parse =
        let (data, _) = tag("=")(data)?;
        let (data, _) = multispace0(data)?;

        // parse value, a quoted string or a non-quoted string with no whitespace
        let (data, value) = alt((quoted_string, unquoted_string))(data)?;

        // insert into map
        key_values.insert(key, value);
        head = data;
    }

    Ok((head, ParseMap { map: key_values }))
}

/// A string that can be either a String or a &str
/// This is used to optimize for values that could be either.
/// Most of the time these will be references and we don't need
/// to allocate anything, but if we need to, we can.
#[derive(Debug, Clone)]
pub enum StringOrStr<'a> {
    String(String),
    Str(&'a str),
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
impl AsRef<str> for StringOrStr<'_> {
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
impl StringOrStr<'_> {
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
/// # use rust_satellite::keyvalue::StringOrStr;
/// assert_eq!(StringOrStr::from("John"), StringOrStr::from("John".to_string()));
/// ```
impl PartialEq for StringOrStr<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl Eq for StringOrStr<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyvalue_parser() {
        const DATA: &str =
            "DEVICEID=JohnAughey KEY=14 TYPE=BUTTON  BITMAP=rawdata PRESSED={true,false}";
        let key_values = ParseMap::try_from(DATA).unwrap();
        let mut keys = key_values.keys().map(|s| s.to_owned()).collect::<Vec<_>>();
        keys.sort();

        assert_eq!(keys, vec!["BITMAP", "DEVICEID", "KEY", "PRESSED", "TYPE",]);
    }

    #[test]
    fn test_keyvalue_quoted_value() {
        const DATA: &str = "key=\"value\"";
        let key_values = ParseMap::try_from(DATA).unwrap();
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }

    #[test]
    fn test_keyvalue_parser_empty() {
        const DATA: &str = "  ";
        let key_values = ParseMap::try_from(DATA).unwrap();
        assert_eq!(key_values.len(), 0);
    }

    #[test]
    fn test_keyvalue_parser_leading_space() {
        const DATA: &str = "  key=value";
        let key_values = ParseMap::try_from(DATA).unwrap();
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }

    #[test]
    fn test_keyvalue_parser_trailing_space() {
        const DATA: &str = "key=value  ";
        let key_values = ParseMap::try_from(DATA).unwrap();
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }

    #[test]
    fn test_keyvalue_parser_multi_inbetween() {
        const DATA: &str = " key=value  foo=bar ";
        let key_values = ParseMap::try_from(DATA).unwrap();
        assert_eq!(key_values.len(), 2);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
        assert_eq!(key_values.get("foo").unwrap(), "bar".into());
    }

    #[test]
    fn test_keyvalue_backslash_value() {
        const DATA: &str = "key=\"value\\\"\"";
        let key_values = ParseMap::try_from(DATA).expect(&format!("Properly parsed {}", DATA));
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value\"".into());
    }

    #[test]
    fn test_keyvalue_space_after_equals() {
        const DATA: &str = "key = value";
        let key_values = ParseMap::try_from(DATA).expect(&format!("Properly parsed {}", DATA));
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }
}
