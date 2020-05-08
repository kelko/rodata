#![warn(rust_2018_idioms)]

#[macro_use]
extern crate clap;

use rodata::convert::{Converter, json::JsonConverter, xml::XmlConverter, csv::CsvConverter};
use rodata::model::{EntitySetQuery, FunctionQuery, EntityIndividualQuery};
use rodata::provider::entity_set::EntitySetIterator;
use rodata::provider::entity_individual::EntityIndividualLoader;
use rodata::provider::function::FunctionCaller;
use rodata::writer::FileWriter;
use clap::ArgMatches;

//"https://services.odata.org/v4/TripPinServiceRW/People"

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let options = clap_app!(rodata =>
        (version: "0.1")
        (author: "Gregor :kelko: Karzelek")
        (about: "Rust OData Client")
        (@subcommand entityset =>
            (about: "Loads an OData Entity Set. Can select output columns, filter and/or sort")
            (@arg select: --select +takes_value "List of fields to query (a.k.a. $select)")
            (@arg filter: --filter +takes_value "Filter for the query (a.k.a $filter)")
            (@arg order: --order-by +takes_value "List of fields to use for ordering/sorting (a.k.a $orderby)")
            (@arg username: -u --username +takes_value "Username")
            (@arg password: -p --password +takes_value "Password")
            (@arg format: -f --format +takes_value "Format (csv, xml, json; default: csv)")
            (@arg output: -o --output +takes_value "File name of the Ouput. `-` for stdout (default)")
            (@arg ENTITYSETURL: +required "The full URL to the OData entityset (without $filter, $select, etc.)")
        )
        (@subcommand entity =>
            (about: "Loads a single OData Entity.")
            (@arg username: -u --username +takes_value "Username")
            (@arg password: -p --password +takes_value "Password")
            (@arg format: -f --format +takes_value "Format (csv, xml, json; default: csv)")
            (@arg output: -o --output +takes_value "File name of the Ouput. `-` for stdout (default)")
            (@arg ENTITYURL: +required "The full URL to the OData entity (including ID parameters!)")
        )
        /*(@subcommand model =>
            (about: "Loads the model of the OData service.")
            (@arg username: -u --username +takes_value "Username")
            (@arg password: -p --password +takes_value "Password")
            (@arg format: -f --format +takes_value "Format (markdown, xsd; default: markdown)")
            (@arg output: -o --output +takes_value "File name of the Ouput. `-` for stdout (default)")
            (@arg ENTITYURL: +required "The full URL to the OData entityset")
            (@arg ID: +required "The identifier of the entity to fetch'")
        )*/
        (@subcommand function =>
            (about: "Calls an OData function.")
            (@arg select: -s --select +takes_value "List of fields to include in the export. Separate with comma")
            (@arg username: -u --username +takes_value "Username")
            (@arg password: -p --password +takes_value "Password")
            (@arg format: -f --format +takes_value "Format (csv, xml, json; default: csv)")
            (@arg output: -o --output +takes_value "File name of the Output. `-` for stdout (default)")
            (@arg FUNCTIONURL: +required "The full URL to the OData function (including all function parameters!)")
        )
    ).get_matches();


    match options.subcommand() {
        ("entityset", entity_options) => load_entity_set(entity_options.expect("Missing required entity set parameters")).await,
        ("entity", entity_options) => load_individual_entity(entity_options.expect("Missing required single entity parameters")).await,
        ("function", function_options) => call_function(function_options.expect("Missing required function call parameters")).await,
        _ => panic!("Invalid call")
    }
}

async fn load_entity_set(options: &ArgMatches<'_>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let output_format = options.value_of("format").map(|value| value.to_string().to_lowercase());
    let query = EntitySetQuery {
        entityset_url: options.value_of("ENTITYSETURL").expect("Missing required parameter ENTITYSETURL").to_string(),
        select: options.value_of("select").map(|value| value.to_string()),
        filters: options.value_of("filter").map(|value| value.to_string()),
        order_by: options.value_of("order").map(|value| value.to_string()),
        username: options.value_of("username").map(|value| value.to_string()),
        password: options.value_of("password").map(|value| value.to_string())
    };
    let out_file = options.value_of_os("output").unwrap_or(std::ffi::OsStr::new("-"));

    let entity_iterator = EntitySetIterator::new();
    let odata_receiver = entity_iterator.iterate_entity_set(query);

    let converter = load_result_converter(output_format);
    let mut writer = FileWriter::new(out_file);

    let (output_sender, output_receiver) = FileWriter::setup_channel();
    converter.convert(odata_receiver, output_sender);
    writer.write(output_receiver).await;

    Ok(())
}

fn load_result_converter(output_format: Option<String>) -> Box<dyn Converter> {
    if let Some(format_value) = output_format {
        match format_value.as_str() {
            "xml" => return Box::new(XmlConverter::new()),
            "json" => return Box::new(JsonConverter::new()),
            _ => ()
        };
    }

    Box::new(CsvConverter::new())
}

async fn load_individual_entity(options: &ArgMatches<'_>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let output_format = options.value_of("format").map(|value| value.to_string().to_lowercase());
    let query = EntityIndividualQuery {
        entity_url: options.value_of("ENTITYURL").expect("Missing required parameter ENTITYURL").to_string(),
        username: options.value_of("username").map(|value| value.to_string()),
        password: options.value_of("password").map(|value| value.to_string())
    };
    let out_file = options.value_of_os("output").unwrap_or(std::ffi::OsStr::new("-"));

    let entity_loader = EntityIndividualLoader::new();
    let odata_receiver = entity_loader.load_individual(query);

    let converter = load_result_converter(output_format);
    let mut writer = FileWriter::new(out_file);

    let (output_sender, output_receiver) = FileWriter::setup_channel();
    converter.convert(odata_receiver, output_sender);
    writer.write(output_receiver).await;

    Ok(())
}

async fn call_function(options: &ArgMatches<'_>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let output_format = options.value_of("format").map(|value| value.to_string().to_lowercase());
    let query = FunctionQuery {
        function_url: options.value_of("FUNCTIONURL").expect("Missing required parameter FUNCTIONURL").to_string(),
        username: options.value_of("username").map(|value| value.to_string()),
        password: options.value_of("password").map(|value| value.to_string())
    };
    let out_file = options.value_of_os("output").unwrap_or(std::ffi::OsStr::new("-"));

    let function_caller = FunctionCaller::new();
    let odata_receiver = function_caller.call_function(query);
    
    let converter = load_result_converter(output_format);
    let mut writer = FileWriter::new(out_file);

    let (output_sender, output_receiver) = FileWriter::setup_channel();
    converter.convert(odata_receiver, output_sender);
    writer.write(output_receiver).await;

    Ok(())
}
