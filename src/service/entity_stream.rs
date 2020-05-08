use std::{thread, time};
use futures::stream::Stream;
use futures::channel::mpsc::Sender;
use bytes::Bytes;
use crate::json_stream::token::JsonToken;
use crate::json_stream::stream::TokenIterator;
use crate::model::{Token, Value, ValuePath, ValuePosition, MyError};

pub enum RootEntityType {
    Array,
    Object,
    Value
}


pub struct EntityStreamer {
    path: ValuePath,
    index : Option<usize>,
    sender: Sender<Token>,
    root_entity : RootEntityType
}

impl EntityStreamer {
    pub fn new(sender: Sender<Token>, root_entity : RootEntityType) -> Self {
        let mut instance = EntityStreamer { sender, root_entity, path: ValuePath::new(), index: None};
        instance.begin();

        return instance;
    }

    fn begin(&mut self) {
        match self.root_entity {
            RootEntityType::Array => {
                self.force_send_message_into_stream(Token { path: ValuePath::new(), value: Value::StartArray });
                self.start_index();
            }
            RootEntityType::Object => self.force_send_message_into_stream(Token { path: ValuePath::new(), value: Value::StartObject }),
            RootEntityType::Value => ()
        }        
    }

    pub async fn stream_content<T>(&mut self, stream: &mut crate::json_stream::stream::Stream<T>) -> Result<(), MyError>
    where T: Stream<Item = reqwest::Result<Bytes>> + Send + Unpin {
        loop {
            if let Some(json_content) =  stream.get() {
                match json_content {
                    JsonToken::JsKey(key) => {
                        self.apply_key(key.into_raw_str().to_owned());
                    },
                    JsonToken::StartArray => {
                        self.apply_index();
                        self.send_message_into_stream(Token { path: self.path.clone(), value: Value::StartArray });
                        self.start_index();
                        
                    },
                    JsonToken::StartObject => {
                        self.apply_index();
                        self.send_message_into_stream(Token { path: self.path.clone(), value: Value::StartObject });
                        self.index = None;
                    },
                    JsonToken::JsNull => {
                        self.apply_index();
                        self.send_message_into_stream(Token { path: self.path.clone(), value: Value::None });
                        self.leave_nesting();
                    },
                    JsonToken::JsNumber(value) => {
                        self.apply_index();
                        self.send_message_into_stream(Token { path: self.path.clone(), value: Value::Number(value.to_owned()) });
                        self.leave_nesting();
                    },
                    JsonToken::JsString(value) => {
                        self.apply_index();
                        self.send_message_into_stream(Token { path: self.path.clone(), value: Value::String(value.into_raw_str().to_owned()) });
                        self.leave_nesting();
                    },
                    JsonToken::JsBoolean(value) => {
                        self.apply_index();
                        self.send_message_into_stream(Token { path: self.path.clone(), value: Value::Boolean(value) });
                        self.leave_nesting();
                    },
                    JsonToken::EndObject => {
                        if self.end_of_scope() {
                            break
                        }

                        self.send_message_into_stream(Token { path: self.path.clone(), value: Value::EndObject });
                        self.leave_nesting();
                    },
                    JsonToken::EndArray => {
                        if self.end_of_scope() {
                            break
                        }

                        self.send_message_into_stream(Token { path: self.path.clone(), value: Value::EndArray });
                        self.leave_nesting();
                    },
                }
            } else {
                return Err(MyError { message: format!("Premature end of content at position {}", self.path.get_path_string()) });
            }

            stream.advance().await?;
        };

        Ok(())
    }

    fn end_of_scope(&self) -> bool {
        self.path.is_empty()
    }

    fn apply_key(&mut self, key: String) {
        self.path.push(ValuePosition::Key(key));
    }

    fn apply_index(&mut self) {
        // keys are explicit. They are set via JsonToken::JsKey
        // index is implicit, calculated by occurrence in array
        // need to keep track of it myself
        if let Some(index_value) = self.index {
            self.path.push(ValuePosition::Index(index_value));
        } 
    }

    fn leave_nesting(&mut self) {
        let previous = self.path.pop();
        
        if let Some(ValuePosition::Index(index_value)) = previous {
            self.index = Some(index_value + 1);
        } else {
            self.index = None;
        }
    }

    fn start_index(&mut self) {
        self.index = Some(0);
    }

    fn should_send(&mut self) -> bool {
        let has_odata_key = self.path.iter().any(|path_part| {
            if let ValuePosition::Key(key_value) = path_part {
                if key_value.starts_with("@odata.") {
                    return true;
                }
            }

            false
        });

        !has_odata_key
    }

    fn send_message_into_stream(&mut self, message: Token) {
        if !self.should_send() {
            return;
        }

        self.force_send_message_into_stream(message);
    }

    fn force_send_message_into_stream(&mut self, message: Token) {
        let mut message = Some(message);
        while message.is_some() {
            match self.sender.try_send(message.take().unwrap()) {
                Ok(_) => (),
                Err(e) => {
                    message = Some(e.into_inner());
                    thread::sleep(time::Duration::from_millis(50));
                }
            }
        }
    }
}

impl Drop for EntityStreamer {
    fn drop(&mut self) {
        match self.root_entity {
            RootEntityType::Array => self.force_send_message_into_stream(Token { path: ValuePath::new(), value: Value::EndArray }),
            RootEntityType::Object => self.force_send_message_into_stream(Token { path: ValuePath::new(), value: Value::EndObject }),
            RootEntityType::Value => ()
        }
        self.sender.disconnect();
    }
}
