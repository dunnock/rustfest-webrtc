use structopt::StructOpt;
use tokio::prelude::*;
use tungstenite::Error as WsError;
use tungstenite::Message as WsMessage;
use rand::prelude::*;
use anyhow::{anyhow, bail, Context};
use std::sync::{Arc, Weak};
use gst::gst_element_error;
use gst::prelude::*;

#[derive(Debug,StructOpt)]
struct Args {
    #[structopt(short,long,default_value="wss://webrtc.nirbheek.in:8443")]
    server: url::Url,
    peer_id: u32
}

// Strong reference to our application state
#[derive(Debug, Clone)]
struct App(Arc<AppInner>);

#[derive(Debug)]
struct AppInner {
    args: Args,
    pipeline: gst::Pipeline
}

#[derive(Debug, Clone)]
struct AppWeak(Weak<AppInner>);

impl std::ops::Deref for App {
    type Target = AppInner;

    fn deref(&self) -> &AppInner {
        &self.0
    }
}

impl AppWeak {
    fn upgrade(&self) -> Option<App> {
        self.0.upgrade().map(App)
    }
}

impl App {
    fn new(args: Args) -> Result<Self, anyhow::Error> {
        // Create the GStreamer pipeline
        let pipeline = gst::parse_launch(
                "videotestsrc pattern=ball ! videoconvert ! autovideosink \
                 audiotestsrc ! audioconvert ! autoaudiosink")?;

        let pipeline = pipeline.downcast::<gst::Pipeline>().expect("not a pipeline");

        // Asynchronously set the pipeline to Playing
        pipeline.call_async(|pipeline| {
            // If this fails, post an error on the bus so we exit
            if pipeline.set_state(gst::State::Playing).is_err() {
                gst_element_error!(
                    pipeline,
                    gst::LibraryError::Failed,
                    ("Failed to set pipeline to Playing")
                );
            }
        });

        Ok(App(Arc::new(AppInner { pipeline, args })))
    }
    fn downgrade(&self) -> AppWeak {
        AppWeak(Arc::downgrade(&self.0))
    }
    fn handle_websocket_message(&self, msg: String) {
        println!("got message {}", msg)
    }
}

async fn run(
        args: Args, 
        ws: impl Sink<WsMessage, Error=WsError> + Stream<Item=Result<WsMessage, WsError>>
    ) -> Result<(), anyhow::Error> 
{
    let (mut ws_sink, ws_stream) = ws.split();
    // Fuse the Stream, required for the select macro
    let mut ws_stream = ws_stream.fuse();

    let app = App::new(args)?;

    loop {
        let ws_msg = futures::select! {
            ws_msg = ws_stream.select_next_some() => {
                match ws_msg? {
                    WsMessage::Close(_) => {
                        println!("peer disconnected");
                        break
                    },
                    WsMessage::Ping(data) => Some(WsMessage::Pong(data)),
                    WsMessage::Pong(data) => None,
                    WsMessage::Binary(_) => None,
                    WsMessage::Text(text) => {
                        app.handle_websocket_message(text);
                        None
                    }
                }
            },
            complete => break,
        };

        // If there's a message to send out, do so now
        if let Some(ws_msg) = ws_msg {
            ws_sink.send(ws_msg).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    println!("Hello, world!");
    let args = Args::from_args();
    gst::init()?;

    //let url = url::Url::parse(&args.server)?;
    let (mut ws, _) = tokio_tungstenite::connect_async(args.server.clone()).await?;

    let our_id: u16 = rand::thread_rng().gen_range(10, 10_000);
    println!("Registering id {} with server", our_id);
    ws.send(WsMessage::Text(format!("HELLO {}", our_id))).await?;

    let msg = ws.next().await
        .ok_or_else(|| anyhow!("didn't receive anything"))??;

    if msg != WsMessage::Text("HELLO".into()) {
        bail!("Server did not return HELLO")
    }

    ws.send(WsMessage::Text(format!("SESSION {}", args.peer_id))).await?;

    run(args, ws).await
}
