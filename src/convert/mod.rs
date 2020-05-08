pub mod csv;
pub mod xml;
pub mod json;

use std::{thread, time};
use futures::channel::mpsc::{Sender, Receiver};
use crate::model::Token;

pub trait Converter {
    fn convert(&self, entity_stream : Receiver<Token>, output : Sender<Box<String>>);
}


pub(crate) fn send_line_to_writer<T>(line: T, sender: &mut Sender<Box<String>>, newline: &str) 
where T: Into<String> {
    let line_content = line.into();
    send_message_to_writer(format!("{}{}", line_content, newline), sender);
}

pub(crate) fn send_message_to_writer<T>(message: T, sender: &mut Sender<Box<String>>)
where T: Into<String> {
    let message_content = message.into();

    let mut message_pointer = Some(Box::new(message_content));
    while message_pointer.is_some() {
        match sender.try_send(message_pointer.take().unwrap()) {
            Ok(_) => (),
            Err(e) => {
                message_pointer = Some(e.into_inner());
                thread::sleep(time::Duration::from_millis(50));
            }
        }
    }
}
