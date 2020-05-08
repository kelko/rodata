#[derive(Default,Debug)]
pub struct EntitySetQuery
{
    pub entityset_url: String,
    pub select: Option<String>,
    pub filters: Option<String>,
    pub order_by: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>
}

impl EntitySetQuery {
    pub fn new(entityset_url: String) -> EntitySetQuery {
        EntitySetQuery { entityset_url, ..Default::default() }
    }

    pub fn has_options(&self) -> bool {
        self.select.is_some() || self.filters.is_some() || self.order_by.is_some()
    }
}

#[derive(Default,Debug)]
pub struct EntityIndividualQuery
{
    pub entity_url: String,
    pub username: Option<String>,
    pub password: Option<String>
}

impl EntityIndividualQuery {
    pub fn new(entity_url: String) -> EntityIndividualQuery {
        EntityIndividualQuery { entity_url, ..Default::default() }
    }
}

#[derive(Default,Debug)]
pub struct FunctionQuery
{
    pub function_url: String,
    pub username: Option<String>,
    pub password: Option<String>
}

impl FunctionQuery {
    pub fn new(function_url: String) -> FunctionQuery {
        FunctionQuery { function_url, ..Default::default() }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ValuePosition {
    Key(String),
    Index(usize)
}

#[derive(Clone, PartialEq, Eq)]
pub struct ValuePath {
    steps: Vec<ValuePosition>
}

impl From<Vec<ValuePosition>> for ValuePath {

    fn from(steps: Vec<ValuePosition>) -> Self {
        ValuePath { steps }
    }
}

impl IntoIterator for ValuePath {
    type Item = ValuePosition;
    type IntoIter = <Vec::<ValuePosition> as IntoIterator>::IntoIter;
    
    fn into_iter(self) -> <Self as std::iter::IntoIterator>::IntoIter {
        self.steps.into_iter()
     }
}

impl From<ValuePosition> for ValuePath {
    fn from(start_position: ValuePosition) -> Self {
        ValuePath { steps: vec![start_position] }
    }
}

impl ValuePath {
    pub fn new() -> Self {
        Self { steps: Vec::<ValuePosition>::new() }
    }

    pub fn stringify_path(path: &Vec<ValuePosition>) -> String {
        let mut first = true;
        let string_parts : Vec<String> = path.into_iter().map(|position| {
            return match position {
                ValuePosition::Key(key_value) => {
                    let result = if first {
                        key_value.to_owned()
                    } else {
                        format!(".{}", key_value)
                    };

                    first = false;
                    result
                }
                ValuePosition::Index(index_value) => {
                    first = false;
                    format!("[{}]", index_value)
                }
            }
        }).collect();
        
        string_parts.join("")
    }

    pub fn get_path_string(&self) -> String {
        Self::stringify_path(&self.steps)
    }

    pub fn build_key_path(&self) -> ValuePath {
        ValuePath { steps: self.steps.iter().filter(|position| match position { ValuePosition::Key(_) => true, _ => false }).cloned().collect() }
    }

    pub fn top_most(&self) -> Option<ValuePosition> {
        if let Some(position) = self.steps.last() {
            Some(position.clone())

        } else {
            return None
        }
    }

    pub fn push(&mut self, next_position: ValuePosition) {
        self.steps.push(next_position);
    }

    pub fn pop(&mut self) -> Option<ValuePosition> {
        self.steps.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn current_level(&self) -> usize {
        self.steps.len()
    }

    pub fn parent(&self) -> Option<Self> {
        if self.is_empty() {
            return None;
        }

        let end_index = self.steps.len() - 1;
        Some(Self { steps: self.steps[..end_index].to_vec() })
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ValuePosition> {
        self.steps.iter()
    }

    pub fn reset(&mut self) {
        self.steps.clear();
    }
}

impl std::fmt::Debug for ValuePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.get_path_string())
    }
}

impl std::fmt::Display for ValuePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.get_path_string())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MyError {
    pub(crate) message: String
}

impl std::fmt::Debug for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::fmt::Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<reqwest::Error> for MyError {
    fn from(_: reqwest::Error) -> Self { 
        MyError{ message: "Some reqwest stuff went wrong".to_owned()}
    }
}

impl From<crate::json_stream::stream::Error> for MyError {

    fn from(source_error: crate::json_stream::stream::Error) -> Self {
        match source_error {
            crate::json_stream::stream::Error::IoError(_) => MyError { message: "I/O-Error occurred".to_owned() },
            crate::json_stream::stream::Error::DecodeError(content) => {
                match content {
                    crate::json_stream::decode::DecodeError::InvalidUnicodeEscape(payload) => MyError { message: format!("Decode-Error: String contains a sequence {:x} which is an invalid unicode code point", payload).to_owned() },
                    crate::json_stream::decode::DecodeError::NeedsMore => MyError { message: "Decode-Error: Needs more".to_owned() },
                    crate::json_stream::decode::DecodeError::UnexpectedEndOfStream => MyError { message: "Decode-Error: More input needed to finish parse, but input bytes marked as end of stream".to_owned() },
                    crate::json_stream::decode::DecodeError::InvalidUtf8 => MyError { message: "Decode-Error: Invalid UTF-8".to_owned() },
                    crate::json_stream::decode::DecodeError::UnexpectedByte(payload) => MyError { message: format!("Decode-Error: found an invalid byte: {:x}", payload).to_owned() },
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Value {
    /// The start of an object, a.k.a. '{'
    StartObject,
    /// The end of an object, a.k.a. '}'
    EndObject,
    /// The start of an array, a.k.a. '['
    StartArray,
    /// The end of an object, a.k.a. ']'
    EndArray,
    /// The token 'null'
    None,
    /// Either 'true' or 'false'
    Boolean(bool),
    /// A number, unparsed. i.e. '-123.456e-789'
    Number(String),
    /// A string in a value context.
    String(String),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::StartObject => f.write_str("{"),
            Value::EndObject => f.write_str("}"),
            Value::StartArray => f.write_str("["),
            Value::EndArray => f.write_str("]"),
            Value::None => f.write_str("null"),
            Value::Boolean(true) => f.write_str("true"),
            Value::Boolean(false) => f.write_str("false"),
            Value::Number(value) => f.write_str(value),
            Value::String(value) => f.write_str(value),            
        }
    }
}


#[derive(Clone, PartialEq, Eq)]
pub struct Token {
    pub path: ValuePath,
    /// the actual content at this path
    pub value: Value,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{} ⇒ {}", self.path, self.value))
    }
}
