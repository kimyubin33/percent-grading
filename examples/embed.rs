//! Run with `cargo run --locked --example embed`; no HTTP or provider call.
use percent_grading::service::{self, Operation, Request, ServiceContext};

fn main() {
    let request = Request {
        api_version: "percent-grading-api-v1".into(),
        request_id: "rust-embed-example".into(),
        payload: Operation::Compile {
            source: include_str!("../policies/apple.aha").into(),
        },
    };
    let response = service::dispatch(&ServiceContext::default(), &request);
    println!("{}", serde_json::to_string_pretty(&response).unwrap());
}
