use std::fs::File;
use std::io::{prelude::*, Write, stdout, BufWriter};
use std::path::Path;
use futures::channel::mpsc::{channel, Sender, Receiver};
use futures::stream::StreamExt;
use futures::executor::block_on;

pub struct FileWriter {
    output: Box<dyn Write>
}

impl FileWriter {
    const CHANNEL_BUFFER_SIZE: usize = 1_000_000;
    const WRITE_BUFFER_SIZE: usize = 1_048_576;

    pub fn setup_channel() -> (Sender<Box<String>>, Receiver<Box<String>>) {
        channel::<Box<String>>(FileWriter::CHANNEL_BUFFER_SIZE)
    }

    pub fn new(out_file: &std::ffi::OsStr) -> FileWriter {
        let output : Box<dyn Write> = if out_file == std::ffi::OsStr::new("-") {
            Box::new(BufWriter::with_capacity(FileWriter::WRITE_BUFFER_SIZE, stdout()))

        } else { 
            Box::new(BufWriter::with_capacity(FileWriter::WRITE_BUFFER_SIZE, File::create(Path::new(out_file)).expect("Could not create output file")))
        };

        FileWriter { output }
    }

    pub async fn write(&mut self, receiver: Receiver<Box<String>>) {
        let mut failed = false;
        let running_foreach = receiver.for_each(move |next_object| {
            if failed {
                return futures::future::ready(());
            }

            match self.output.write((*next_object).as_bytes()) {
                Ok(_) => (),
                Err(e) => {
                    failed = true;
                    eprintln!("{}", e);
                    return futures::future::ready(());
                }
            }
            futures::future::ready(())
        });

        block_on(running_foreach)
    }
}
