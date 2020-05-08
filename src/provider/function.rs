use futures::channel::mpsc::{ channel, Sender, Receiver};
use futures::stream::Stream;
use bytes::Bytes;
use crate::model::{FunctionQuery, Token, MyError};
use crate::service::url::SingleUrlCaller;
use crate::service::entity_stream::{EntityStreamer, RootEntityType};
use crate::json_stream::token::JsonToken;
use crate::json_stream::stream::TokenIterator;

pub struct FunctionCaller {}
impl FunctionCaller {
    const BUFFER_SIZE: usize = 1_000_000;

    pub fn new() -> FunctionCaller {
        FunctionCaller {}
    }

    pub fn call_function<T: Into<FunctionQuery>>(self, query: T) -> Receiver<Token> {
        let function_query = query.into();
        let url_caller = SingleUrlCaller::new(function_query.function_url, function_query.username, function_query.password);
        let (sender, receiver) = channel::<Token>(FunctionCaller::BUFFER_SIZE);

        self.run_in_background(url_caller, sender);
        
        return receiver;
    }

    fn run_in_background(&self, url_caller: SingleUrlCaller, sender: Sender<Token>) {
        tokio::spawn(async move {
            let mut collector = FunctionResultCollector::new(sender);

            match url_caller.call().await {
                Ok(response) => {
                    match collector.stream_odata_object(response).await {
                        Ok(_) => (),
                        Err(err) => {
                            eprintln!("{}", err.message);
                            return futures::future::ready(());
                        }
                    }
                },
                Err(err) => {
                    eprintln!("{}", err.message);
                    return futures::future::ready(());
                }
            }

            futures::future::ready(())
        });
    }
}

struct FunctionResultCollector {
    stream: EntityStreamer
}

impl FunctionResultCollector {
    fn new(sender: Sender<Token>) -> Self {
        FunctionResultCollector { stream: EntityStreamer::new(sender, RootEntityType::Array) }
    }

    async fn stream_odata_object<T>(&mut self, odata_response: T) -> Result<(), MyError>
    where T: Stream<Item = reqwest::Result<Bytes>> + Send + Unpin {
        let mut error : Option<MyError> = None;

        let mut stream = crate::json_stream::stream::Stream::from_stream(odata_response)?;
        stream.advance().await?; // start of object
        stream.advance().await?; //hopefully a key
        loop {
            if let Some(json_content) = stream.get() {

                match json_content {
                    JsonToken::JsKey(key_val) => {
                        match key_val.into_raw_str() {

                            "value" => {
                                stream.advance().await?;
                                if let Some(value_token) = stream.get() {

                                    if value_token == JsonToken::StartArray {
                                        stream.advance().await?;
                                        self.stream.stream_content(&mut stream).await?;

                                    } else {
                                        error = Some(MyError { message: "Expected an array for key 'value'".to_owned() });
                                        break;
                                    }

                                } else {
                                    error = Some(MyError { message: "Expected an array for key 'value'".to_owned() });
                                    break;
                                };
                            },
                            _ => {
                                stream.advance().await?; // skip the value for that key
                            }
                        }
                    },
                    JsonToken::EndObject => {
                        break;
                    },
                    _ => {
                        error = Some(MyError { message: format!("Invalid top level JSON structure of response: {:?}", json_content) });
                        break;
                    }
                }
            } else {
                break;
            };

            stream.advance().await?;
        }
        
        if let Some(occurred_error) = error {
            return Err(occurred_error);
        }

        return Ok(());
    }
}
