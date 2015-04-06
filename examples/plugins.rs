#[macro_use]
extern crate rustful;

use std::sync::RwLock;
use std::error::Error;

use rustful::{Server, TreeRouter, Context, Response, Log, Handler};
use rustful::plugin::{PluginContext, ResponsePlugin, ResponseAction, ContextPlugin, ContextAction};
use rustful::response::ResponseData;
use rustful::Method::Get;
use rustful::StatusCode;
use rustful::header::Headers;

fn say_hello(mut context: Context, mut response: Response) {
    //Take the name of the JSONP function from the query variables
    if let Some(jsonp_name) = context.query.remove("jsonp") {
        response.plugin_storage().insert(JsonpFn(jsonp_name));
    }

    let person = match context.variables.get("person") {
        Some(name) => &name[..],
        None => "stranger"
    };

    if let Err(e) = response.into_writer().send(format!("{{\"message\": \"Hello, {}!\"}}", person))  {
        //There is not much we can do now
        context.log.note(&format!("could not send hello: {}", e.description()));
    }
}

//Dodge an ICE, related to functions as handlers.
struct HandlerFn(fn(Context, Response));

impl Handler for HandlerFn {
    fn handle_request(&self, context: Context, response: Response) {
        self.0(context, response);
    }
}

fn main() {
    println!("Visit http://localhost:8080 or http://localhost:8080/Peter (if your name is Peter) to try this example.");
    println!("Append ?jsonp=someFunction to get a JSONP response.");

    let mut router = TreeRouter::new();
    insert_routes!{
        &mut router => {
            "print" => {
                "/" => Get: HandlerFn(say_hello),
                ":person" => Get: HandlerFn(say_hello)
            }
        }
    };

    let server_result = Server::new()
           .handlers(router)
           .port(8080)

            //Log path, change path, log again
           .with_context_plugin(RequestLogger::new())
           .with_context_plugin(PathPrefix::new("print"))
           .with_context_plugin(RequestLogger::new())

           .with_response_plugin(Jsonp)

           .run();

    match server_result {
        Ok(_server) => {},
        Err(e) => println!("could not start server: {}", e.description())
    }
}

struct RequestLogger {
    counter: RwLock<u32>
}

impl RequestLogger {
    pub fn new() -> RequestLogger {
        RequestLogger {
            counter: RwLock::new(0)
        }
    }
}

impl ContextPlugin for RequestLogger {
    ///Count requests and log the path.
    fn modify(&self, ctx: PluginContext, context: &mut Context) -> ContextAction {
        *self.counter.write().unwrap() += 1;
        ctx.log.note(&format!("Request #{} is to '{}'", *self.counter.read().unwrap(), context.path));
        ContextAction::next()
    }
}


struct PathPrefix {
    prefix: &'static str
}

impl PathPrefix {
    pub fn new(prefix: &'static str) -> PathPrefix {
        PathPrefix {
            prefix: prefix
        }
    }
}

impl ContextPlugin for PathPrefix {
    ///Append the prefix to the path
    fn modify(&self, _ctx: PluginContext, context: &mut Context) -> ContextAction {
        context.path = format!("/{}{}", self.prefix.trim_matches('/'), context.path);
        ContextAction::next()
    }
}

struct JsonpFn(String);

struct Jsonp;

impl ResponsePlugin for Jsonp {
    fn begin(&self, ctx: PluginContext, status: StatusCode, headers: Headers) -> (StatusCode, Headers, ResponseAction) {
        //Check if a JSONP function is defined and write the beginning of the call
        let output = if let Some(&JsonpFn(ref function)) = ctx.storage.get() {
            Some(format!("{}(", function))
        } else {
            None
        };

        (status, headers, ResponseAction::next(output))
    }

    fn write<'a>(&'a self, _ctx: PluginContext, bytes: Option<ResponseData<'a>>) -> ResponseAction {
        ResponseAction::next(bytes)
    }

    fn end(&self, ctx: PluginContext) -> ResponseAction {
        //Check if a JSONP function is defined and write the end of the call
        let output = ctx.storage.get::<JsonpFn>().map(|_| ");");
        ResponseAction::next(output)
    }
}