use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::sync::{
    mpsc,
    mpsc::{Receiver, Sender},
    Arc, Mutex, MutexGuard,
};
use std::thread;

struct Conn {
    stream: TcpStream,
    id: usize,
}

type Message = (String, usize);

impl Conn {
    fn new(stream: TcpStream, id: usize) -> Conn {
        Conn { stream, id }
    }

    fn lock(conns: &Arc<Mutex<Vec<Conn>>>) -> MutexGuard<'_, Vec<Conn>> {
        match conns.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("WARNING: Mutex is poisoned. Recovering the lock.");
                poisoned.into_inner()
            }
        }
    }
}

fn clone(stream: &TcpStream) -> TcpStream {
    match stream.try_clone() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("ERROR: could not clone stream.");
            std::process::exit(1);
        }
    }
}

fn handle_stream(
    mut stream: TcpStream,
    conns: Arc<Mutex<Vec<Conn>>>,
    id: usize,
    tx: Sender<Message>,
) {
    let conn = Conn::new(clone(&stream), id);
    Conn::lock(&conns).push(conn);

    let mut buf = [0; 512];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => {
                println!("Connection {} disconnected", id);
                break;
            }
            Ok(n) => tx
                .send((String::from_utf8_lossy(&buf[..n]).to_string(), id))
                .unwrap(),
            Err(e) => eprintln!("ERROR read: {}", e),
        }
    }

    Conn::lock(&conns).retain(|conn| conn.id != id);
}

fn broadcast(rx: Receiver<Message>, conns: Arc<Mutex<Vec<Conn>>>) {
    for message in rx {
        let msg = message.0.as_bytes();
        let mut conns = Conn::lock(&conns);
        for conn in conns.iter_mut() {
            if message.1 != conn.id {
                conn.stream.write_all(msg).unwrap();
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    let conns: Arc<Mutex<Vec<Conn>>> = Arc::new(Mutex::new(Vec::new()));

    let (tx, rx) = mpsc::channel();

    let conns_dup = Arc::clone(&conns);
    thread::spawn(move || broadcast(rx, conns_dup));

    let mut id = 0;

    println!("Listening on port 8080");

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        println!("New connection.");

        let conns = Arc::clone(&conns);
        let tx = tx.clone();
        thread::spawn(move || {
            handle_stream(stream, conns, id, tx);
        });

        id += 1;
    }
}
