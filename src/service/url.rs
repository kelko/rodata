use bytes::Bytes;
use crate::model::MyError;

async fn call_url(url: &str, _username: &Option<String>, _password: &Option<String>) -> Result<impl futures::stream::Stream<Item = reqwest::Result<Bytes>>, MyError> {
    let url = String::from(url);

    let result = reqwest::get(&url).await?.bytes_stream();
    Ok(result)
}

#[derive(Clone)]
pub struct SingleUrlCaller {
    url: String,
    username: Option<String>,
    password: Option<String>
}

impl SingleUrlCaller {
    pub fn new(url: String, username: Option<String>, password: Option<String>) -> SingleUrlCaller {
        SingleUrlCaller { url, username, password }
    }

    pub(crate) async fn call(&self) -> Result<impl futures::stream::Stream<Item = reqwest::Result<Bytes>>, MyError> {
        let content  = call_url(&self.url, &self.username, &self.password).await?;

        Ok(content)
    }
}

#[derive(Clone)]
pub struct MultiUrlCaller {
    starting_url: String,
    username: Option<String>,
    password: Option<String>
}

impl MultiUrlCaller{
    pub fn new(starting_url: String, username: Option<String>, password: Option<String>) -> MultiUrlCaller {
        MultiUrlCaller { starting_url, username, password }
    }

    pub(crate) fn starting_link_marker(&self) -> &String {
        &self.starting_url
    }

    pub(crate) async fn next(&self, odata_next_link: &Option<String>) -> Result<Option<impl futures::stream::Stream<Item = reqwest::Result<Bytes>>>, MyError> {
        return match odata_next_link {
            Some(link) => {
                let content  = call_url(&link, &self.username, &self.password).await?;

                Ok(Some(content))
            },
            None =>  {
                Ok(None)
            }
        };
    }
}
