use std::collections::HashMap;

pub use anyhow::Result;
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while},
    character::complete::multispace0,
    Finish, IResult,
};

pub struct ParseMap<'a> {
    map: HashMap<String, StringOrStr<'a>>,
}

impl<'a> ParseMap<'a> {
    pub fn get(&self, key: &str) -> Result<StringOrStr<'a>> {
        self.map
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Key {} not found", key))
    }

    #[cfg(test)]
    fn keys(&self) -> std::collections::hash_map::Keys<String, StringOrStr> {
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

        // look at the next char, if it's a quote, we're done, consume it and return
        let (data, maybe_quote) = take(1usize)(data)?;
        if maybe_quote == "\"" {
            // if we've accumulated strings so far, add this to the end and return,
            // otherwise we return the string reference and don't allocate.
            match accum {
                Some(accum) => {
                    return Ok((data, StringOrStr::String(accum + value)));
                }
                None => {
                    return Ok((data, StringOrStr::Str(value)));
                }
            }
        }
        // Crap, there's a backslash
        // create an accumulator if we haven't already and append the string we've parsed so far
        accum.get_or_insert_with(|| value.to_string());

        head = data;
    }
}

fn unquoted_string(data: &str) -> IResult<&str, StringOrStr> {
    let (data, value) = take_while(|c: char| !c.is_whitespace())(data)?;
    Ok((data, StringOrStr::Str(value)))
}

fn str_to_key_value(data: &str) -> IResult<&str, ParseMap> {
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
        let (data, key) =
            take_while(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-')(data)?;
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
}
