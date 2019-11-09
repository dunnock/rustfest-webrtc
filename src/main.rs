use structopt::StructOpt;
use tokio::prelude::*;
use tungstenite::Error as WsError;
use tungstenite::Message as WsMessage;
use rand::prelude::*;
use anyhow::{anyhow, bail, Context};

#[derive(Debug,StructOpt)]
struct Args {
    #[structopt(short,long,default_value="wss://webrtc.nirbheek.in:8443")]
    server: url::Url,
}


#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    println!("Hello, world!");
    let args = Args::from_args();

    //let url = url::Url::parse(&args.server)?;
    let (mut ws, _) = tokio_tungstenite::connect_async(args.server).await?;

    let our_id = rand::thread_rng().gen_range(10, 10_000);
    println!("Registering id {} with server", our_id);
    ws.send(WsMessage::Text(format!("HELLO {}", our_id))).await?;

    let msg = ws.next().await
        .ok_or_else(|| anyhow!("didn't receive anything"))??;

    if msg != WsMessage::Text("HELLO".into()) {
        bail!("Server did not return HELLO")
    }
    Ok(())
}
