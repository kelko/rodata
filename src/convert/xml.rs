use futures::channel::mpsc::{Sender, Receiver};
use futures::stream::StreamExt;
use futures::executor::block_on;
use crate::convert::{Converter, send_message_to_writer};
use crate::model::{Token, Value, ValuePosition};

pub struct XmlConverter {
}

impl XmlConverter {
    pub fn new() -> XmlConverter {
        XmlConverter {}
    }

    fn stream_as_xml(token: &Token, output: &mut Sender<Box<String>>) {
        match &token.value {
            Value::None => {
                match token.path.top_most() {
                    Some(ValuePosition::Index(_)) => send_message_to_writer("<value />", output),
                    Some(ValuePosition::Key(key_value)) => send_message_to_writer(format!("<{} />", key_value), output),
                    None => (),
                }
            },
            Value::Boolean(value) => {
                let stringified_value = if *value {
                    "true"
                } else {
                    "false"
                };

                match token.path.top_most() {
                    Some(ValuePosition::Index(_)) => send_message_to_writer(format!("<value>{}</value>", stringified_value), output),
                    Some(ValuePosition::Key(key_value)) => send_message_to_writer(format!("<{}>{}</{}>", key_value, stringified_value, key_value), output),
                    None => send_message_to_writer(stringified_value, output)
                }
            },
            Value::Number(value) | Value::String(value) => {
                match token.path.top_most() {
                    Some(ValuePosition::Index(_)) => send_message_to_writer(format!("<value>{}</value>", value), output),
                    Some(ValuePosition::Key(key_value)) => send_message_to_writer(format!("<{}>{}</{}>", key_value, value, key_value), output),
                    None => send_message_to_writer(value, output)
                }
            },
            Value::StartArray => {
                match token.path.top_most() {
                    Some(ValuePosition::Index(_)) | None => send_message_to_writer("<list>", output),
                    Some(ValuePosition::Key(key_value)) => send_message_to_writer(format!("<{}>", key_value), output)
                }
            },
            Value::StartObject => {
                match token.path.top_most() {
                    Some(ValuePosition::Index(_)) | None => send_message_to_writer("<object>", output),
                    Some(ValuePosition::Key(key_value)) => send_message_to_writer(format!("<{}>", key_value), output)
                }
            },
            Value::EndArray => {
                match token.path.top_most() {
                    Some(ValuePosition::Index(_)) | None => send_message_to_writer("</list>", output),
                    Some(ValuePosition::Key(key_value)) => send_message_to_writer(format!("</{}>", key_value), output)
                }
            },
            Value::EndObject => {
                match token.path.top_most() {
                    Some(ValuePosition::Index(_)) | None => send_message_to_writer("</object>", output),
                    Some(ValuePosition::Key(key_value)) => send_message_to_writer(format!("</{}>", key_value), output)
                }
            }
        }
    }
}

impl Converter for XmlConverter {
    fn convert(&self, entity_stream : Receiver<Token>, mut output: Sender<Box<String>>) {
        tokio::spawn(async move {
            let running_foreach = entity_stream.for_each(move |next_object : Token| {
                XmlConverter::stream_as_xml(&next_object, &mut output);

                futures::future::ready(())
            });
            block_on(running_foreach)
        });
    }
}
