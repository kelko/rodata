use futures::channel::mpsc::{ channel, Sender, Receiver};
use futures::stream::Stream;
use bytes::Bytes;
use crate::model::{EntityIndividualQuery, Token, MyError};
use crate::service::url::SingleUrlCaller;
use crate::service::entity_stream::{EntityStreamer, RootEntityType};
use crate::json_stream::stream::TokenIterator;

pub struct EntityIndividualLoader {}
impl EntityIndividualLoader {
    const BUFFER_SIZE: usize = 1_000_000;

    pub fn new() -> EntityIndividualLoader {
        EntityIndividualLoader {}
    }

    pub fn load_individual<T: Into<EntityIndividualQuery>>(self, query: T) -> Receiver<Token> {
        let entity_individual_query = query.into();
        let url_caller = SingleUrlCaller::new(entity_individual_query.entity_url, entity_individual_query.username, entity_individual_query.password);
        let (sender, receiver) = channel::<Token>(EntityIndividualLoader::BUFFER_SIZE);

        self.run_in_background(url_caller, sender);
        
        return receiver;
    }

    fn run_in_background(&self, url_caller: SingleUrlCaller, sender: Sender<Token>) {
        tokio::spawn(async move {
            let mut reader = EntityReader::new(sender);

            match url_caller.call().await {
                Ok(response) => {
                    match reader.stream_odata_object(response).await {
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

struct EntityReader {
    stream: EntityStreamer
}

impl EntityReader {
    fn new(sender: Sender<Token>) -> Self {
        EntityReader { stream: EntityStreamer::new(sender, RootEntityType::Object) }
    }

    async fn stream_odata_object<T>(&mut self, odata_response: T) -> Result<(), MyError>
    where T: Stream<Item = reqwest::Result<Bytes>> + Send + Unpin {

        let mut stream = crate::json_stream::stream::Stream::from_stream(odata_response)?;
        stream.advance().await?; // start of object (should not be inspected by `stream_content`)
        stream.advance().await?; // first entry in the object
        self.stream.stream_content(&mut stream).await?;

        return Ok(());
    }
}
