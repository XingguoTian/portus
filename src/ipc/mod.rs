use std;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;

extern crate nix;

#[derive(Debug)]
pub struct Error(String);

impl From<nix::Error> for Error {
    fn from(e: nix::Error) -> Error {
        Error(format!("err {}", e))
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error(format!("err {}", e))
    }
}

#[cfg(all(linux))]
pub mod netlink;
pub mod unix;

pub trait Ipc {
    fn send(&self, addr: Option<u16>, msg: &[u8]) -> Result<(), Error>; // Blocking send
    fn recv(&self, msg: &mut [u8]) -> Result<usize, Error>; // Blocking listen
    fn close(&self) -> Result<(), Error>; // Close the underlying sockets
}

pub struct Backend<T: Ipc + Sync> {
    sock: Arc<T>,
    notif_ch: mpsc::Sender<Vec<u8>>,
    close: Arc<std::sync::atomic::AtomicBool>,
}

impl<T: Ipc + Sync> Clone for Backend<T> {
    fn clone(&self) -> Self {
        Backend {
            sock: self.sock.clone(),
            notif_ch: self.notif_ch.clone(),
            close: self.close.clone(),
        }
    }
}

impl<T: Ipc + 'static + Sync + Send> Backend<T> {
    // Pass in a T: Ipc, the Ipc substrate to use.
    // Return a Backend on which to call send_msg
    // and a channel on which to listen for incoming
    pub fn new(sock: T) -> Result<(Backend<T>, mpsc::Receiver<Vec<u8>>), Error> {
        let (tx, rx): (mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>) = mpsc::channel();
        let b = Backend {
            sock: Arc::new(sock),
            notif_ch: tx,
            close: Default::default(),
        };

        b.listen();
        Ok((b, rx))
    }

    // Blocking send.
    pub fn send_msg(&self, addr: Option<u16>, msg: &[u8]) -> Result<(), Error> {
        self.sock.send(addr, msg).map_err(|e| Error::from(e))
    }

    fn listen(&self) {
        let me = self.clone();
        thread::spawn(move || {
            let mut rcv_buf = vec![0u8; 1024];
            while me.close.load(Ordering::SeqCst) {
                let len = match me.sock.recv(rcv_buf.as_mut_slice()) {
                    Ok(l) => l,
                    Err(_) => {
                        //println!("{:?}", e);
                        continue;
                    }
                };

                if len == 0 {
                    continue;
                }

                rcv_buf.truncate(len);
                match me.notif_ch.send(rcv_buf.clone()) {
                    Ok(_) => (),
                    Err(_) => {
                        //println!("{}", e);
                        continue;
                    }
                };
            }
        });
    }
}

impl<T: Ipc + Sync> Drop for Backend<T> {
    fn drop(&mut self) {
        // tell the receive loop to exit
        self.close.store(true, Ordering::SeqCst)
    }
}

#[cfg(test)]
pub mod test;
