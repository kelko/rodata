use futures::stream::Stream;
use futures::channel::mpsc::{ channel, Sender, Receiver};
use bytes::Bytes;
use crate::model::{EntitySetQuery, MyError, Token};
use crate::service::url::MultiUrlCaller;
use crate::service::entity_stream::{EntityStreamer, RootEntityType};
use crate::json_stream::token::JsonToken;
use crate::json_stream::stream::TokenIterator;

pub struct EntitySetIterator {}
impl EntitySetIterator {
    const BUFFER_SIZE: usize = 1_000_000;

    pub fn new() -> EntitySetIterator {
        EntitySetIterator {}
    }

    pub fn iterate_entity_set<T: Into<EntitySetQuery>>(self, query: T) -> Receiver<Token> {
        let entity_set_query = query.into();
        let multi_caller = MultiUrlCaller::new(self.build_full_url(&entity_set_query), entity_set_query.username, entity_set_query.password);
        let (sender, receiver) = channel::<Token>(EntitySetIterator::BUFFER_SIZE);

        self.run_in_background(multi_caller, sender);
        return receiver;
    }

    fn build_full_url(&self, query: &EntitySetQuery) -> String {
        let mut full_url = String::with_capacity(128);
        full_url.push_str(&query.entityset_url);

        if !query.has_options() {
            return full_url;
        }

        let mut ampersand_necessary = false;
        full_url.push('?');

        if let Some(filter) = &query.filters
        {
            full_url.push_str("$filter=");
            full_url.push_str(filter);
            ampersand_necessary = true;
        }

        if let Some(select) = &query.select
        {
            if ampersand_necessary
            {
                full_url.push('&');
            }

            full_url.push_str("$select=");
            full_url.push_str(select);
            ampersand_necessary = true;
        }

        if let Some(order_by) = &query.order_by
        {
            if ampersand_necessary
            {
                full_url.push('&');
            }

            full_url.push_str("$orderby=");
            full_url.push_str(order_by);
        }

        full_url
    }

    fn run_in_background(&self, url_caller: MultiUrlCaller, sender: Sender<Token>) {
        tokio::spawn(async move {
            let mut collector = EntityCollector::new(sender);
            let mut next_url = Some(url_caller.starting_link_marker().clone());

            while next_url.is_some() {
                match url_caller.next(&next_url).await {
                    Ok(Some(response)) => {
                        match collector.stream_odata_objects(response).await {
                            Ok(url) => next_url = url,
                            Err(err) => {
                                eprintln!("{}", err.message);
                                return futures::future::ready(());
                            }
                        }
                    },
                    Ok(None) => break,
                    Err(err) => {
                        eprintln!("{}", err.message);
                        return futures::future::ready(());
                    }
                }
            }

            futures::future::ready(())
        });
    }
}

struct EntityCollector {
    stream: EntityStreamer
}

impl EntityCollector {
    fn new(sender: Sender<Token>) -> Self {
        EntityCollector { stream: EntityStreamer::new(sender, RootEntityType::Array) }
    }

    async fn stream_odata_objects<T>(&mut self, odata_response: T) -> Result<Option<String>, MyError>
    where T: Stream<Item = reqwest::Result<Bytes>> + Send + Unpin {
        let mut error : Option<MyError> = None;
        let mut next : Option<String> = None;

        let mut stream = crate::json_stream::stream::Stream::from_stream(odata_response)?;
        stream.advance().await?; // start of object
        stream.advance().await?; //hopefully a key
        loop {
            if let Some(json_content) = stream.get() {

                match json_content {
                    JsonToken::JsKey(key_val) => {
                        match key_val.into_raw_str() {

                            "@odata.nextLink" => {
                                stream.advance().await?; 
                                if let Some(next_link_token) = stream.get() {

                                    if let JsonToken::JsString(next_link_value) = next_link_token {
                                        next = Some(next_link_value.into_raw_str().to_owned());

                                    } else {
                                        error = Some(MyError { message: "Expected a string value for key '@odata.nextLink'".to_owned() });
                                        break;
                                    };

                                } else {
                                    error = Some(MyError { message: "Expected a value after key '@odata.nextLink'".to_owned() });
                                    break;
                                };
                            },
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

        return Ok(next);
    }
}
