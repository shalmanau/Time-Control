use crate::{bytes, now, Group, Identity, Member, Replica, Store};
use anyhow::{ensure, Context, Result};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

pub const NOISE: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
pub const SERVICE: &str = "_timeledger._tcp.local.";
const MAX_MESSAGE: usize = 16 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Peer {
    pub name: String,
    pub group_id: String,
    pub address: String,
}
#[derive(Clone, Serialize)]
pub struct JoinRequest {
    pub id: String,
    pub name: String,
    pub code: String,
}
#[derive(Clone, Serialize)]
pub struct Status {
    pub port: u16,
    pub peers: Vec<Peer>,
    pub pending: Vec<JoinRequest>,
    pub joining_code: Option<String>,
    pub last_sync: Option<i64>,
    pub error: Option<String>,
    pub active: bool,
}
struct Pending {
    request: JoinRequest,
    decision: Arc<Mutex<Option<bool>>>,
}
pub struct Network {
    pub store: Arc<Mutex<Store>>,
    pub port: u16,
    pub active: AtomicBool,
    peers: Mutex<BTreeMap<String, Peer>>,
    pending: Mutex<Vec<Pending>>,
    joining_code: Mutex<Option<String>>,
    last_sync: Mutex<Option<i64>>,
    error: Mutex<Option<String>>,
    busy: AtomicBool,
}
#[derive(Serialize, Deserialize)]
struct Hello {
    member: Member,
    group: Group,
    mode: String,
    proof: String,
}
#[derive(Serialize, Deserialize)]
struct Reply {
    ok: bool,
    message: String,
    replica: Option<Replica>,
}
impl Network {
    pub fn start(store: Arc<Mutex<Store>>) -> Result<Arc<Self>> {
        Self::start_inner(store, true)
    }
    fn start_inner(store: Arc<Mutex<Store>>, discover: bool) -> Result<Arc<Self>> {
        let listener = TcpListener::bind("0.0.0.0:0")?;
        let port = listener.local_addr()?.port();
        let net = Arc::new(Self {
            store,
            port,
            active: AtomicBool::new(true),
            peers: Mutex::new(BTreeMap::new()),
            pending: Mutex::new(vec![]),
            joining_code: Mutex::new(None),
            last_sync: Mutex::new(None),
            error: Mutex::new(None),
            busy: AtomicBool::new(false),
        });
        let n = net.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if !n.active.load(Ordering::Relaxed) {
                    continue;
                }
                let n = n.clone();
                thread::spawn(move || {
                    if let Err(e) = n.serve(stream) {
                        *n.error.lock().unwrap() = Some(e.to_string());
                    }
                });
            }
        });
        #[cfg(not(target_os = "android"))]
        if discover {
            if let Err(e) = net.discover() {
                *net.error.lock().unwrap() = Some(format!("Nearby discovery unavailable: {e}"));
            }
        }
        #[cfg(target_os = "android")]
        let _ = discover;
        Ok(net)
    }
    pub fn status(&self) -> Status {
        Status {
            port: self.port,
            peers: self.peers.lock().unwrap().values().cloned().collect(),
            pending: self
                .pending
                .lock()
                .unwrap()
                .iter()
                .map(|p| p.request.clone())
                .collect(),
            joining_code: self.joining_code.lock().unwrap().clone(),
            last_sync: *self.last_sync.lock().unwrap(),
            error: self.error.lock().unwrap().clone(),
            active: self.active.load(Ordering::Relaxed),
        }
    }
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }
    pub fn peer(&self, key: String, peer: Option<Peer>) {
        let mut p = self.peers.lock().unwrap();
        if let Some(peer) = peer {
            p.insert(key, peer);
        } else {
            p.remove(&key);
        }
    }
    pub fn approve(&self, id: &str, accept: bool) -> Result<()> {
        let pending = self.pending.lock().unwrap();
        let p = pending
            .iter()
            .find(|p| p.request.id == id)
            .context("Request expired")?;
        *p.decision.lock().unwrap() = Some(accept);
        Ok(())
    }
    pub fn tick(self: &Arc<Self>) {
        if !self.active.load(Ordering::Relaxed) || self.busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let n = self.clone();
        thread::spawn(move || {
            let group = n.store.lock().unwrap().replica().group.id;
            let peers: Vec<Peer> = n
                .peers
                .lock()
                .unwrap()
                .values()
                .filter(|p| p.group_id == group)
                .cloned()
                .collect();
            for p in peers {
                if let Err(e) = n.connect(&p.address, false) {
                    *n.error.lock().unwrap() = Some(format!("{}: {e}", p.name));
                }
            }
            n.busy.store(false, Ordering::SeqCst);
        });
    }
    pub fn join_peer(self: &Arc<Self>, address: String) -> Result<()> {
        ensure!(
            !self.busy.swap(true, Ordering::SeqCst),
            "Synchronization is busy; try again shortly"
        );
        let n = self.clone();
        thread::spawn(move || {
            if let Err(e) = n.connect(&address, true) {
                *n.error.lock().unwrap() = Some(e.to_string());
            }
            *n.joining_code.lock().unwrap() = None;
            n.busy.store(false, Ordering::SeqCst);
        });
        Ok(())
    }
    fn hello(&self, identity: &Identity, hash: &[u8], mode: &str) -> Result<Hello> {
        Ok(Hello {
            member: identity.member(),
            group: self.store.lock().unwrap().replica().group,
            mode: mode.into(),
            proof: hex::encode(identity.key()?.sign(hash).to_bytes()),
        })
    }
    fn verify_hello(h: &Hello, remote: &[u8], hash: &[u8]) -> Result<()> {
        ensure!(
            h.member.id == h.member.public_key && hex::decode(&h.member.noise_key)? == remote,
            "Device identity mismatch"
        );
        VerifyingKey::from_bytes(&bytes(&h.member.public_key)?)?
            .verify(hash, &Signature::from_bytes(&bytes(&h.proof)?))?;
        Ok(())
    }
    fn trust(&self, h: &Hello) -> Result<()> {
        let store = self.store.lock().unwrap();
        let own = store.replica().group;
        ensure!(
            h.group.id == own.id && h.group.root == own.root,
            "Device is in a different group"
        );
        h.group.validate()?;
        let member = h
            .group
            .members
            .iter()
            .find(|m| m.id == h.member.id)
            .context("Device is not a group member")?;
        ensure!(
            member.noise_key == h.member.noise_key,
            "Untrusted device key"
        );
        Ok(())
    }
    fn serve(&self, stream: TcpStream) -> Result<()> {
        let identity = self.store.lock().unwrap().identity();
        let (mut channel, remote, hash) = Channel::handshake(stream, &identity, false)?;
        let h: Hello = channel.receive()?;
        Self::verify_hello(&h, &remote, &hash)?;
        channel.send(&self.hello(&identity, &hash, "reply")?)?;
        if h.mode == "join" {
            ensure!(
                self.pending.lock().unwrap().len() < 8,
                "Too many pending requests"
            );
            ensure!(
                h.group.members.len() == 1,
                "Joining device already has a group"
            );
            let request = JoinRequest {
                id: hex::encode(&hash[..12]),
                name: h.member.name.clone(),
                code: code(&hash),
            };
            let decision = Arc::new(Mutex::new(None));
            self.pending.lock().unwrap().push(Pending {
                request: request.clone(),
                decision: decision.clone(),
            });
            let start = Instant::now();
            let accepted = loop {
                if let Some(d) = *decision.lock().unwrap() {
                    break d;
                }
                if start.elapsed() > Duration::from_secs(120)
                    || !self.active.load(Ordering::Relaxed)
                {
                    break false;
                }
                thread::sleep(Duration::from_millis(200));
            };
            self.pending
                .lock()
                .unwrap()
                .retain(|p| p.request.id != request.id);
            if !accepted {
                channel.send(&Reply {
                    ok: false,
                    message: "Joining was declined or timed out".into(),
                    replica: None,
                })?;
                return Ok(());
            }
            let replica = {
                let mut s = self.store.lock().unwrap();
                s.admit(h.member)?;
                s.replica()
            };
            channel.send(&Reply {
                ok: true,
                message: String::new(),
                replica: Some(replica),
            })?;
        } else {
            ensure!(h.mode == "sync", "Unknown request");
            self.trust(&h)?;
            channel.send(&Reply {
                ok: true,
                message: String::new(),
                replica: None,
            })?;
        }
        let replica: Replica = channel.receive()?;
        let output = {
            let mut store = self.store.lock().unwrap();
            store.merge(replica)?;
            store.replica()
        };
        channel.send(&output)?;
        *self.last_sync.lock().unwrap() = Some(now());
        *self.error.lock().unwrap() = None;
        Ok(())
    }
    fn connect(&self, address: &str, joining: bool) -> Result<()> {
        let address: SocketAddr = address.parse().context("Invalid local device address")?;
        ensure!(
            is_local(address.ip()),
            "Only local-network addresses are allowed"
        );
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
        let identity = self.store.lock().unwrap().identity();
        let (mut channel, remote, hash) = Channel::handshake(stream, &identity, true)?;
        channel.send(&self.hello(&identity, &hash, if joining { "join" } else { "sync" })?)?;
        let hello: Hello = channel.receive()?;
        Self::verify_hello(&hello, &remote, &hash)?;
        if joining {
            *self.joining_code.lock().unwrap() = Some(code(&hash));
        } else {
            self.trust(&hello)?;
        }
        let reply: Reply = channel.receive()?;
        ensure!(reply.ok, "{}", reply.message);
        if joining {
            self.store
                .lock()
                .unwrap()
                .join(reply.replica.context("Missing group data")?)?;
        }
        let local = self.store.lock().unwrap().replica();
        channel.send(&local)?;
        let incoming = channel.receive()?;
        self.store.lock().unwrap().merge(incoming)?;
        *self.last_sync.lock().unwrap() = Some(now());
        *self.error.lock().unwrap() = None;
        Ok(())
    }
    #[cfg(not(target_os = "android"))]
    fn discover(self: &Arc<Self>) -> Result<()> {
        use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
        let daemon = ServiceDaemon::new()?;
        let identity = self.store.lock().unwrap().identity();
        let instance = format!("TimeLedger-{}", &identity.id[..12]);
        let group = self.store.lock().unwrap().replica().group.id;
        let info = ServiceInfo::new(
            SERVICE,
            &instance,
            &format!("{instance}.local."),
            "",
            self.port,
            [("name", identity.name.as_str()), ("group", group.as_str())].as_slice(),
        )?
        .enable_addr_auto();
        daemon.register(info)?;
        let receiver = daemon.browse(SERVICE)?;
        let n = self.clone();
        thread::spawn(move || {
            let mut advertised = group;
            loop {
                if let Ok(event) = receiver.recv_timeout(Duration::from_secs(2)) {
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            if !info.get_fullname().starts_with(&instance) {
                                if let Some(ip) =
                                    info.get_addresses().iter().find(|ip| ip.is_ipv4())
                                {
                                    let peer = Peer {
                                        name: info
                                            .get_property_val_str("name")
                                            .unwrap_or("Device")
                                            .into(),
                                        group_id: info
                                            .get_property_val_str("group")
                                            .unwrap_or("")
                                            .into(),
                                        address: SocketAddr::new(*ip, info.get_port()).to_string(),
                                    };
                                    n.peer(info.get_fullname().into(), Some(peer));
                                }
                            }
                        }
                        ServiceEvent::ServiceRemoved(_, name) => n.peer(name, None),
                        _ => {}
                    }
                }
                let current = n.store.lock().unwrap().replica().group.id;
                if current != advertised {
                    if let Ok(info) = ServiceInfo::new(
                        SERVICE,
                        &instance,
                        &format!("{instance}.local."),
                        "",
                        n.port,
                        [
                            ("name", identity.name.as_str()),
                            ("group", current.as_str()),
                        ]
                        .as_slice(),
                    ) {
                        let _ = daemon.register(info.enable_addr_auto());
                        advertised = current;
                    }
                }
            }
        });
        Ok(())
    }
}
fn is_local(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
        }
    }
}
fn code(hash: &[u8]) -> String {
    let digest = Sha256::digest(hash);
    format!(
        "{:06}",
        u32::from_be_bytes(digest[..4].try_into().unwrap()) % 1_000_000
    )
}
struct Channel {
    stream: TcpStream,
    noise: snow::TransportState,
}
fn write_frame(stream: &mut TcpStream, data: &[u8]) -> Result<()> {
    ensure!(data.len() <= 65535, "Frame too large");
    stream.write_all(&(data.len() as u16).to_be_bytes())?;
    stream.write_all(data)?;
    Ok(())
}
fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut len = [0; 2];
    stream.read_exact(&mut len)?;
    let mut data = vec![0; u16::from_be_bytes(len) as usize];
    stream.read_exact(&mut data)?;
    Ok(data)
}
impl Channel {
    fn handshake(
        mut stream: TcpStream,
        identity: &Identity,
        initiator: bool,
    ) -> Result<(Self, Vec<u8>, Vec<u8>)> {
        stream.set_read_timeout(Some(Duration::from_secs(135)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        let private = hex::decode(&identity.noise_private)?;
        let builder = snow::Builder::new(NOISE.parse()?).local_private_key(&private)?;
        let mut noise = if initiator {
            builder.build_initiator()?
        } else {
            builder.build_responder()?
        };
        let mut buf = [0u8; 65535];
        if initiator {
            let n = noise.write_message(&[], &mut buf)?;
            write_frame(&mut stream, &buf[..n])?;
            noise.read_message(&read_frame(&mut stream)?, &mut buf)?;
            let n = noise.write_message(&[], &mut buf)?;
            write_frame(&mut stream, &buf[..n])?;
        } else {
            noise.read_message(&read_frame(&mut stream)?, &mut buf)?;
            let n = noise.write_message(&[], &mut buf)?;
            write_frame(&mut stream, &buf[..n])?;
            noise.read_message(&read_frame(&mut stream)?, &mut buf)?;
        }
        let remote = noise
            .get_remote_static()
            .context("Missing remote identity")?
            .to_vec();
        let hash = noise.get_handshake_hash().to_vec();
        Ok((
            Self {
                stream,
                noise: noise.into_transport_mode()?,
            },
            remote,
            hash,
        ))
    }
    fn send<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let data = serde_json::to_vec(value)?;
        ensure!(
            data.len() <= MAX_MESSAGE,
            "History exceeds synchronization limit"
        );
        let mut buf = [0u8; 65535];
        let n = self
            .noise
            .write_message(&(data.len() as u64).to_be_bytes(), &mut buf)?;
        write_frame(&mut self.stream, &buf[..n])?;
        for chunk in data.chunks(60000) {
            let n = self.noise.write_message(chunk, &mut buf)?;
            write_frame(&mut self.stream, &buf[..n])?;
        }
        Ok(())
    }
    fn receive<T: serde::de::DeserializeOwned>(&mut self) -> Result<T> {
        let mut buf = [0u8; 65535];
        let n = self
            .noise
            .read_message(&read_frame(&mut self.stream)?, &mut buf)?;
        ensure!(n == 8, "Invalid message header");
        let length = u64::from_be_bytes(buf[..8].try_into()?) as usize;
        ensure!(length <= MAX_MESSAGE, "Message too large");
        let mut data = Vec::with_capacity(length);
        while data.len() < length {
            let n = self
                .noise
                .read_message(&read_frame(&mut self.stream)?, &mut buf)?;
            ensure!(n > 0 && data.len() + n <= length, "Invalid message length");
            data.extend_from_slice(&buf[..n]);
        }
        Ok(serde_json::from_slice(&data)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(name: &str) -> Arc<Network> {
        Network::start_inner(
            Arc::new(Mutex::new(
                Store::open(":memory:", name.into(), "UTC").unwrap(),
            )),
            false,
        )
        .unwrap()
    }
    fn pair(a: &Arc<Network>, b: &Arc<Network>) {
        let member = b.store.lock().unwrap().identity().member();
        a.store.lock().unwrap().admit(member).unwrap();
        let replica = a.store.lock().unwrap().replica();
        b.store.lock().unwrap().join(replica).unwrap();
    }
    #[test]
    fn encrypted_transport_merges_and_repeats_without_duplicates() {
        let a = node("A");
        let b = node("B");
        pair(&a, &b);
        {
            let mut s = a.store.lock().unwrap();
            let c = s.add_category("Work", 1000).unwrap();
            s.save_entry(None, c.id, 100, 200, 1000).unwrap();
        }
        b.connect(&format!("127.0.0.1:{}", a.port), false).unwrap();
        b.connect(&format!("127.0.0.1:{}", a.port), false).unwrap();
        assert_eq!(b.store.lock().unwrap().snapshot().entries.len(), 1);
    }
    #[test]
    fn approved_join_shares_code_and_keeps_offline_entries() {
        let a = node("A");
        let b = node("B");
        {
            let mut s = b.store.lock().unwrap();
            let c = s.add_category("Reading", 1000).unwrap();
            s.save_entry(None, c.id, 100, 200, 1000).unwrap();
        }
        let n = b.clone();
        let address = format!("127.0.0.1:{}", a.port);
        let client = thread::spawn(move || n.connect(&address, true));
        let start = Instant::now();
        loop {
            if let Some(p) = a
                .status()
                .pending
                .first()
                .filter(|_| b.status().joining_code.is_some())
            {
                assert_eq!(Some(p.code.clone()), b.status().joining_code);
                a.approve(&p.id, true).unwrap();
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(20));
        }
        client.join().unwrap().unwrap();
        assert_eq!(a.store.lock().unwrap().snapshot().entries.len(), 1);
        assert_eq!(
            b.store.lock().unwrap().replica().group.id,
            a.store.lock().unwrap().replica().group.id
        );
    }
    #[test]
    fn unapproved_device_cannot_read_records() {
        let a = node("A");
        let b = node("B");
        assert!(b.connect(&format!("127.0.0.1:{}", a.port), false).is_err());
        assert_ne!(
            a.store.lock().unwrap().replica().group.id,
            b.store.lock().unwrap().replica().group.id
        );
    }
    #[test]
    fn simultaneous_sync_does_not_deadlock() {
        let a = node("A");
        let b = node("B");
        pair(&a, &b);
        let x = a.clone();
        let y = b.clone();
        let p = a.port;
        let q = b.port;
        let t = thread::spawn(move || x.connect(&format!("127.0.0.1:{q}"), false));
        y.connect(&format!("127.0.0.1:{p}"), false).unwrap();
        t.join().unwrap().unwrap();
    }
    #[test]
    fn interrupted_connection_leaves_database_intact() {
        let a = node("A");
        {
            let mut s = a.store.lock().unwrap();
            s.add_category("Sleep", 1000).unwrap();
        }
        let before = a.store.lock().unwrap().replica().operations.len();
        let mut stream = TcpStream::connect(("127.0.0.1", a.port)).unwrap();
        stream.write_all(&[0, 10, 1, 2]).unwrap();
        drop(stream);
        thread::sleep(Duration::from_millis(30));
        assert_eq!(a.store.lock().unwrap().replica().operations.len(), before);
    }
}
