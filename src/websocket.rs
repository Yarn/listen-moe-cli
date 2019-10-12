
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::protocol::Role;

use hyper::{Client, Request};
use hyper_tls::HttpsConnector;

use tokio_tungstenite::tungstenite::{Message, Error as WsError};

use futures::channel::mpsc;
use futures::stream::StreamExt;
use futures::future::Either;

use crate::shared;

type WsStream = WebSocketStream<hyper::upgrade::Upgraded>;

pub async fn connect(kpop: bool) -> WsStream {
    
    let https = HttpsConnector::new().unwrap();
    let client = Client::builder().build::<_, hyper::Body>(https);
    
    let url = if kpop {
        "https://listen.moe/kpop/gateway_v2"
    } else {
        "https://listen.moe/gateway_v2"
    };
    
    let mut req = Request::builder();
    let req = req
        .method("GET")
        .uri(url)
        .header("User-Agent", shared::USER_AGENT)
        
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        
        .body(hyper::Body::empty())
        .expect("request builder");
    
    let res = client.request(req).await.unwrap();
    
    let body = res.into_body();
    
    let upgrade: hyper::upgrade::Upgraded = body.on_upgrade().await.unwrap();
    
    let ws = WebSocketStream::from_raw_socket(upgrade, Role::Client, None).await;
    
    ws
}

pub struct WsSender {
    chan: mpsc::UnboundedSender<Message>,
}

impl Clone for WsSender {
    fn clone(&self) -> Self {
        WsSender {
            chan: self.chan.clone(),
        }
    }
}

pub struct WsReceiver {
    chan: mpsc::UnboundedReceiver<Result<Message, WsError>>,
}

impl WsReceiver {
    pub async fn get_json(&mut self) -> serde_json::Value {
        let text = match self.chan.next().await.unwrap().unwrap() {
            Message::Text(text) => {
                text
            }
            msg => {
                panic!("unexpected {:?}", msg)
            }
        };
        
        let value = serde_json::from_str(&text).unwrap();
        
        value
    }
}

pub async fn wrap_ws(mut ws: WsStream) -> (WsSender, WsReceiver) {
    let (in_send, in_recv) = mpsc::unbounded();
    let (out_send, mut out_recv) = mpsc::unbounded();
    
    tokio::spawn(async move {
        loop {
            let either = futures::future::select(ws.next(), out_recv.next()).await;
            match either {
                Either::Left((msg, _)) => {
                    let msg: Result<Message, _> = msg.unwrap();
                    // println!("recv {:?}", msg);
                    in_send.unbounded_send(msg).unwrap();
                }
                Either::Right((msg, _)) => {
                    // println!("send {:?}", msg);
                    ws.send(msg.unwrap()).await.unwrap();
                }
            }
        }
    });
    
    let send = WsSender { chan: out_send };
    let mut recv = WsReceiver { chan: in_recv };
    
    let init_msg = recv.get_json().await;
    // println!("init {:?}", init_msg);
    let heartbeat = init_msg.get("d").unwrap().get("heartbeat").unwrap().as_u64().unwrap();
    // println!("heartbeat {}", heartbeat);
    
    let send_b = send.clone();
    tokio::spawn(async move {
        loop {
            tokio::timer::delay_for(core::time::Duration::from_millis(heartbeat)).await;
            send_b.chan.unbounded_send(Message::Text(r#"{"op": 9}"#.into())).unwrap();
        }
    });
    
    (send, recv)
}
