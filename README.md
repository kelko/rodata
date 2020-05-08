rodata - Rust OData Client
==========================

About
-----

`rodata` is first and foremost a command line tool to query OData services without having to bother with the details of the protocol. The content is output to either CVS (partly flattening hierarchical structure) or to JSON or maybe even XML.

It is implemented in a way so that the backend parts of `rodata` can also be used inside other applications/libs.



Usage: CLI
-----

Build `roc` locally, afterwards it can be used as following:

```sh
# Load the EntitySet "People" from the odata.org example and convert to CSV
./roc entityset https://services.odata.org/V4/TripPinServiceRW/People

# Convert "People" to JSON and write it into a file
./roc entityset -f json -o out.json https://services.odata.org/V4/TripPinServiceRW/People

# Only load the fields "FirstName" and "Username"
./roc entityset --select FirstName,UserName https://services.odata.org/V4/TripPinServiceRW/People

# Load the details of one single entity
./roc entity "https://services.odata.org/V4/TripPinServiceRW/People('russellwhyte')"
```


Usage: Backend Code
-----

From the beginning rodata was meant for CLI usage (see above) but also to give developers some building blocks to create
own apps/libs to handle OData.
One of the main ideas is to handle different phases of processing in parallel to speed up the process as a whole. 
Instead of waiting for the service to finish serving the JSON content and then re-formatting you have classes which handle 
calling the service and applying minimal re-formatting and push the content into a [`futures::channel::mpsc`](https://docs.rs/futures/0.3.16/futures/channel/mpsc/index.html).
Another block reads that individual message parts and performs further processing/converting. 

The `roc` CLI makes use of those building blocks by reading the service and then sends it to a formatter to create the 
string parts which then are pushed into a second mpsc taken by a "writer" to handle the file IO.


License
-------

rodata is provided under the MIT license. See [LICENSE](LICENSE).



Acknowledgment
--------------

This code is based upon and uses other code published under open source licenses. See [ACKNOWLEDGEMENTS](ACKNOWLEDGEMENTS).