use anyhow::{anyhow, bail, Error};
use argh::FromArgs;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;

const SERVER_ENVVAR: &str = "BEELAY_SERVER";

#[derive(FromArgs, PartialEq, Debug)]
/// Beelay CLI client
struct Args {
    #[argh(subcommand)]
    command: SubCommands,

    #[argh(option, short = 's', long = "server")]
    /// beelay server address
    server: Option<String>
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum SubCommands {
    Get(GetCommand),
    Set(SetCommand),
    List(ListCommand)
}

#[derive(FromArgs, PartialEq, Debug)]
/// get switch state
#[argh(subcommand, name = "get")]
struct GetCommand {
    #[argh(positional)]
    /// switch name
    switch_name: String,
}

#[derive(FromArgs, PartialEq, Debug)]
/// set switch state
#[argh(subcommand, name = "set")]
struct SetCommand {
    #[argh(positional)]
    /// switch name
    switch_name: String,

    #[argh(positional)]
    /// state ("on" or "off")
    state: String,

    #[argh(option, short = 'd', long = "delay")]
    /// state change delay
    delay: Option<String>
}

#[derive(FromArgs, PartialEq, Debug)]
/// list switches
#[argh(subcommand, name = "list")]
struct ListCommand { }

#[derive(Deserialize)]
struct SwitchStateResponse {
    // pub _status: String,
    pub state: String,
    pub transitioning: String
}

#[derive(Deserialize)]
struct ErrorResponse {
    // pub _status: String,
    pub error_message: String
}

#[derive(Deserialize)]
struct SwitchesResponse {
    pub switches: Vec<String>,
    // pub _pretty_names: HashMap<String, String>,
    // pub _filters: HashMap<String, Vec<String>>
}

fn main() {
    let args: Args = argh::from_env();

    let server_addr = 
        fix_server_addr(
            match args.server {
                Some(server) => server,
                None => match std::env::var(SERVER_ENVVAR) {
                    Ok(server) => server,
                    Err(_) => "http://localhost:9999".to_string()
                }
            }
        );

    let client = Client::new();
    let res = match args.command {
        SubCommands::Get(args) => get_switch(client, server_addr, args.switch_name),
        SubCommands::Set(args) => set_switch(client, server_addr, args.switch_name, args.state),
        SubCommands::List(_) => list_switches(client, server_addr),
    };

    if let Err(err) = res {
        eprintln!("Error during beelay request:");
        eprintln!("    {err}")
    }
}

fn get_switch(client: Client, server_addr: String, switch_name: String) -> Result<(), Error> {
    let resp = 
        client.get(to_switch_url(server_addr, switch_name))
            .send()?;

    if !resp.status().is_success() {
        handle_bad_status_code(resp)
    }
    else {
        print_switch_state_response(resp)
    }
}

fn set_switch(client: Client, server_addr: String, switch_name: String, state: String) -> Result<(), Error> {
    let resp = 
        client.post(to_switch_url(server_addr, switch_name))
            .query(&[("state", &state)])
            .send()?;

    if !resp.status().is_success() {
        handle_bad_status_code(resp)
    }
    else {
        print_switch_state_response(resp)
    }
}

fn print_switch_state_response(resp: Response) -> Result<(), Error> {
    let text = resp.text()?;
    let resp: SwitchStateResponse = serde_json::from_str(&text)
        .map_err(|err| anyhow!("Failed to parse response: {}", err))?;

    println!("state         : {}", resp.state);
    println!("transitioning : {}", resp.transitioning);

    Ok(())
}

fn list_switches(client: Client, server_addr: String) -> Result<(), Error> {
    let resp = 
        client.get(to_switches_url(server_addr))
            .send()?;
    
    if !resp.status().is_success() {
        handle_bad_status_code(resp)
    }
    else {
        let resp: SwitchesResponse = serde_json::from_str(&resp.text()?)
            .map_err(|err| anyhow!("Failed to parse response: {}", err))?;

        println!("Switch list:");
        for switch in resp.switches {
            println!("    {switch}");
        }

        Ok(())
    }
}

fn fix_server_addr(mut server_addr: String) -> String {
    if !server_addr.starts_with("http://") {
        server_addr = format!("http://{server_addr}")
    }

    if !server_addr.ends_with('/') {
        server_addr = format!("{server_addr}/")
    }

    server_addr
}

fn to_switch_url(server_addr: String, switch_name: String) -> String {
    let switch_name = match switch_name.contains(" ") {
        true => switch_name.replace(" ", "%20"),
        false => switch_name
    };

    format!("{server_addr}api/switch/{switch_name}")
}

fn to_switches_url(server_addr: String) -> String {
    format!("{server_addr}api/switches/")
}

fn handle_bad_status_code(resp: Response) -> Result<(), Error> {
    let status_code = resp.status();
    if let Ok(text) = resp.text() {
        match get_error_message(&text) {
            Ok(err_msg) => bail!("{} response: {}", status_code, err_msg),
            Err(err) => bail!("Could not retrieve error message for {} response: {}", status_code, err)
        };
    }
    else {
        bail!("Failed to get text body from response")
    }
}

fn get_error_message(resp_text: &str) -> Result<String, Error> {
    let error_resp: ErrorResponse = serde_json::from_str(&resp_text)?;
    Ok(error_resp.error_message)
}
