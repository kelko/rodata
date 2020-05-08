use std::collections::HashSet;
use futures::channel::mpsc::{Sender, Receiver};
use futures::stream::StreamExt;
use futures::executor::block_on;
use crate::model::{Token, ValuePath, ValuePosition, Value};
use crate::convert::{ Converter, send_line_to_writer };

pub struct CsvConverter<'a> {
    delimiter: &'a str,
    newline: &'a str
}

impl<'a> CsvConverter<'a> {
    pub fn new() -> CsvConverter<'a> {
        CsvConverter { delimiter: ";", newline: "\r\n" }
    }
}

impl<'a> Converter for CsvConverter<'a> {
    fn convert(&self, entity_stream : Receiver<Token>, mut output: Sender<Box<String>>) {
        let newline = self.newline.to_owned();
        let delimiter = self.delimiter.to_owned();
        
        tokio::spawn(async move {
            let mut heavylifter = HeavyliftConverter::new(&delimiter, &newline, &mut output);
            let mut first = true;

            let running_foreach = entity_stream.for_each(move |next_object| {
                let object_ready = heavylifter.buffer_object(next_object);

                if !object_ready {
                    return futures::future::ready(());
                }

                
                if first {
                    heavylifter.send_header();    
                }

                first = false;
                heavylifter.send_object();                

                futures::future::ready(())
            });

            block_on(running_foreach)
        });
    }
}

struct HeavyliftConverter<'a> {
    single_object: bool,
    output: &'a mut Sender<Box<String>>,
    delimiter: &'a str,
    newline: &'a str,
    object_in_progress: std::vec::Vec<(String,String)>,
    value_in_progress: std::vec::Vec<String>,
    known_entities: HashSet<String>,
}

impl<'a> HeavyliftConverter<'a> {
    fn new(delimiter: &'a str, newline: &'a str, output: &'a mut Sender<Box<String>>) -> Self {
        HeavyliftConverter { delimiter, newline, output, object_in_progress: vec![], value_in_progress: vec![], known_entities: HashSet::<String>::new(), single_object: false }
    }

    fn buffer_object(&mut self, token: Token) -> bool {
        match token.path.current_level() {
            0 => {
                match token.value {
                    Value::StartObject => {
                        self.single_object = true;
                        self.object_in_progress = vec![];
                    },
                    Value::EndObject => return true,
                    Value::StartArray => self.single_object = false,
                    Value::EndArray => (),
                    _ => panic!("Invalid token '{}' at {}", token.value, token.path)
                }
            },
            1 => {
                if self.single_object {
                    self.buffer_content(token);

                } else {
                    match token.value {
                        Value::StartObject => {
                            self.object_in_progress = vec![];
                        }
                        Value::EndObject => return true,
                        _ => panic!("Invalid token '{}' at {}", token.value, token.path)
                    };
                }                
            },
            2 => {
                if self.single_object {
                    self.build_complex_value(token);
                } else {                    
                    self.buffer_content(token);
                }
                
            },
            _ => {
                self.build_complex_value(token);
            }
        }

        false
    }

    fn buffer_content(&mut self, token: Token) {
        if let Some(ValuePosition::Key(key)) = token.path.top_most() {
            match token.value {
                Value::Boolean(true) => self.object_in_progress.push((key, "true".to_owned())),
                Value::Boolean(false) => self.object_in_progress.push((key, "false".to_owned())),
                Value::Number(value) => self.object_in_progress.push((key, value)),
                Value::String(value) => self.object_in_progress.push((key, format!("\"{}\"", value))),
                Value::None => self.object_in_progress.push((key, "".to_owned())),
                Value::StartArray | Value::StartObject => self.start_complex_value(),
                Value::EndObject | Value::EndArray => {
                    let value = self.finish_complex_value();
                    self.object_in_progress.push((key, value))
                }
            };
        }
    }

    fn start_complex_value(&mut self) {
        self.value_in_progress =  vec![];
        self.known_entities = HashSet::<String>::new();
    }

    fn build_complex_value(&mut self, token: Token) {
        match token.value {
            Value::Boolean(true) => self.push_valuepart(&token.path, "true"),
            Value::Boolean(false) => self.push_valuepart(&token.path, "false"),
            Value::Number(value) => self.push_valuepart(&token.path, value),
            Value::String(value) => self.push_valuepart(&token.path, value),
            Value::None => self.push_valuepart(&token.path, "null"),
            Value::StartArray | Value::EndArray => (),
            Value::StartObject => self.push_valuepart(&token.path, "("),
            Value::EndObject => {
                self.value_in_progress.push(")".to_owned());
                self.finished_object_as_valuepart(&token.path);
            }
        }
    }

    fn push_valuepart<T>(&mut self, path: &ValuePath, value: T) where T : Into<String> {
        match path.top_most().expect("There has to be some path to come here") {
            ValuePosition::Index(index) => {
                let stringified = self.build_array_valuepart(index, value);
                self.value_in_progress.push(stringified);

            },
            ValuePosition::Key(key_value) => {
                let stringified = self.build_object_valuepart(path, &key_value, value);
                self.value_in_progress.push(stringified);
            }
        }
    }
    
    fn valuepart_needs_array_separator(&self, index: usize) -> bool {
        index > 0
    }

    fn build_array_valuepart<T>(&mut self, index: usize, value: T)  -> String
    where T: Into<String>{
        let prefix = if self.valuepart_needs_array_separator(index) {
            " / "
        } else {
            ""
        };

        
        format!("{}{}", prefix, value.into())
    }

    fn valuepart_needs_object_separator(&self, path: &ValuePath) -> bool {
        if let Some(parent) = path.parent() {
            return self.known_entities.contains(&parent.get_path_string())
        }

        false
    }

    fn processed_object_key_in_valuepart(&mut self, path: &ValuePath) {
        if let Some(parent) = path.parent() {
            self.known_entities.insert(parent.get_path_string());
        }
    }

    fn finished_object_as_valuepart(&mut self, path: &ValuePath) {
        self.known_entities.remove(&path.get_path_string());
    }

    fn build_object_valuepart<T>(&mut self, path: &ValuePath, key: &String, value: T) -> String
    where T: Into<String>{
        let prefix = if self.valuepart_needs_object_separator(path) {
            " / "
        } else {
            ""
        };

        let value = format!("{}{}: {}", prefix, key, value.into());
        self.processed_object_key_in_valuepart(path);

        return value;
    }

    fn finish_complex_value(&mut self) -> String{
        return self.value_in_progress.join("")
    }

    fn format_header(&self) -> String {
        self.object_in_progress.iter().map(|(key, _)| key.clone()).collect::<std::vec::Vec<String>>().join(self.delimiter)
    }

    fn format_value_line(&self) -> String {
        self.object_in_progress.iter().map(|(_, value)| value.clone()).collect::<std::vec::Vec<String>>().join(self.delimiter)
    }

    fn send_header(&mut self) {
        send_line_to_writer(self.format_header(), &mut self.output, self.newline);
    }

    fn send_object(&mut self) {
        send_line_to_writer(self.format_value_line(), &mut self.output, self.newline);
    }
}
