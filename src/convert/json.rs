use std::collections::HashSet;
use futures::channel::mpsc::{Sender, Receiver};
use futures::stream::StreamExt;
use futures::executor::block_on;
use crate::convert::{Converter, send_message_to_writer};
use crate::model::{Token, Value, ValuePath, ValuePosition};

enum ProcessableTokenValue {
    ArrayFinishedString(&'static str),
    ObjectFinishedString(&'static str),
    ValueString(String)
}

fn stringify_token_value(token_value: &Value) -> ProcessableTokenValue {
    match token_value {
        Value::None => ProcessableTokenValue::ValueString("null".to_owned()),
        Value::Boolean(true) => ProcessableTokenValue::ValueString("true".to_owned()),
        Value::Boolean(false) => ProcessableTokenValue::ValueString("false".to_owned()),
        Value::Number(value) => ProcessableTokenValue::ValueString(value.clone()),
        Value::String(value) => ProcessableTokenValue::ValueString(format!("\"{}\"", value)),
        Value::StartArray => ProcessableTokenValue::ValueString("[".to_owned()),
        Value::StartObject => ProcessableTokenValue::ValueString("{".to_owned()),
        Value::EndArray => ProcessableTokenValue::ArrayFinishedString("]"),
        Value::EndObject => ProcessableTokenValue::ObjectFinishedString("}"),
    }
}

pub struct JsonConverter {
}

impl JsonConverter {
    pub fn new() -> JsonConverter {
        JsonConverter {}
    }
}

impl Converter for JsonConverter {
    fn convert(&self, entity_stream : Receiver<Token>, mut output: Sender<Box<String>>) {
        tokio::spawn(async move {
            let mut heavylifter = HeavyliftConverter::new(&mut output);
            let running_foreach = entity_stream.for_each(move |next_object : Token| {
                heavylifter.forward_json(&next_object);
            
                futures::future::ready(())
            });
            

            block_on(running_foreach)
        });
    }
}

struct HeavyliftConverter<'a> {
    known_entities: HashSet<String>,
    output: &'a mut Sender<Box<String>>
}

impl<'a> HeavyliftConverter<'a> {
    fn new(output: &'a mut Sender<Box<String>>) -> Self {
        HeavyliftConverter { known_entities: HashSet::<String>::new(), output}
    }

    
    fn needs_array_separator(&self, index: usize) -> bool {
        index > 0
    }

    fn build_array_value<T>(&mut self, index: usize, value: T)  -> String
    where T: Into<String>{
        let prefix = if self.needs_array_separator(index) {
            ","
        } else {
            ""
        };

        
        format!("{}{}", prefix, value.into())
    }

    fn needs_object_separator(&self, path: &ValuePath) -> bool {
        if let Some(parent) = path.parent() {
            return self.known_entities.contains(&parent.get_path_string())
        }

        false
    }

    fn processed_object_key(&mut self, path: &ValuePath) {
        if let Some(parent) = path.parent() {
            self.known_entities.insert(parent.get_path_string());
        }
    }

    fn finished_object(&mut self, path: &ValuePath) {
        self.known_entities.remove(&path.get_path_string());
    }

    fn build_object_value<T>(&mut self, path: &ValuePath, key: &String, value: T) -> String
    where T: Into<String>{
        let prefix = if self.needs_object_separator(path) {
            ","
        } else {
            ""
        };

        let value = format!("{}\"{}\": {}", prefix, key, value.into());
        self.processed_object_key(path);

        return value;
    }

    fn forward_json(&mut self, token: &Token) {
        let message = match token.path.top_most() {
            Some(ValuePosition::Index(index)) => match stringify_token_value(&token.value) {
                ProcessableTokenValue::ArrayFinishedString(value) => Some(value.to_owned()),
                ProcessableTokenValue::ObjectFinishedString(value) => {
                    self.finished_object(&token.path);
                    Some(value.to_owned())
                }
                ProcessableTokenValue::ValueString(value) => Some(self.build_array_value(index, value))
            }
            Some(ValuePosition::Key(key_value)) => match stringify_token_value(&token.value) {
                ProcessableTokenValue::ArrayFinishedString(value) => Some(value.to_owned()),
                ProcessableTokenValue::ObjectFinishedString(value) => {
                    self.finished_object(&token.path);
                    Some(value.to_owned())
                }
                ProcessableTokenValue::ValueString(value) => Some(self.build_object_value(&token.path, &key_value, value))
            }
            None => match stringify_token_value(&token.value) {
                ProcessableTokenValue::ArrayFinishedString(value) => Some(value.to_owned()),
                ProcessableTokenValue::ObjectFinishedString(value) => Some(value.to_owned()),
                ProcessableTokenValue::ValueString(value) => Some(value)
            }
        };

        if let Some(message_content) = message {
            send_message_to_writer(message_content, self.output);
        }
    }
}
