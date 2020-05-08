/*
use futures::Stream;
use futures::task::{Context, Poll};
use tokio::macros::support::Pin;
use crate::entity_stream::ODataObject;
use std::rc::Rc;
use std::borrow::Borrow;
use std::ops::Deref;

pub struct EntityAttributeFilter<TInner> where TInner : futures::stream::Stream<Item=ODataObject> + Send + Sync  {
    whitelist: Vec<String>,
    inner: Rc<TInner>
}

impl<TInner> EntityAttributeFilter<TInner>  where TInner : futures::stream::Stream<Item=ODataObject> + Send + Sync {
    pub fn new(inner: TInner, whitelist: Vec<String>) -> EntityAttributeFilter<TInner> {
        EntityAttributeFilter { inner: Rc::new(inner), whitelist }
    }

    pub fn from_stringified_whitelist(inner: TInner, concatted_field_list: String) -> EntityAttributeFilter<TInner> {
        EntityAttributeFilter::new(inner, EntityAttributeFilter::<TInner>::split_whitelist(concatted_field_list))
    }

    fn split_whitelist(concatted_field_list: String) -> Vec<String> {
        concatted_field_list.split(",").map(|value| value.to_string()).collect()
    }
}

impl<TInner : futures::stream::Stream> Stream for EntityAttributeFilter<TInner> where TInner : futures::stream::Stream<Item=ODataObject> + Send + Sync   {
    type Item = ODataObject;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inner = Pin::new(self.inner.deref());
        let next = inner.poll_next(cx);

        next.map(|object_option| -> Option<ODataObject> {
            object_option.map(|stream_object| -> ODataObject {
                let mut result = stream_object.clone();
                for key in stream_object.keys() {
                    if !self.whitelist.contains(key) {
                        result.remove(key);
                    }
                }

                result
            })
        })
    }
}

pub struct LimitEntitiesFilter<TInner> where TInner : futures::stream::Stream<Item=ODataObject> + Send + Sync {
    limit: usize,
    current: usize,
    inner: TInner
}

impl<TInner> LimitEntitiesFilter<TInner> where TInner : futures::stream::Stream<Item=ODataObject> + Send + Sync {
    pub fn new(inner: TInner, limit: usize) -> LimitEntitiesFilter<TInner> {
        LimitEntitiesFilter { inner, limit, current: 0 }
    }
}

impl<TInner: futures::stream::Stream> Stream for LimitEntitiesFilter<TInner> where TInner : futures::stream::Stream<Item=ODataObject> + Send + Sync  {
    type Item = ODataObject;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.limit == self.current {
            return Poll::Ready(None);
        }

        self.current += 1;
        self.as_mut().inner.poll_next(cx)
    }
}
*/
