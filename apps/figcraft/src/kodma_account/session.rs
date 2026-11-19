//! The account worker owns its Keychain item and serializes refresh, retry and sign-out.
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{CLIENT_ID, Discovery, Error, ISSUER, LoginGate, agent, fetch_json, login, read_json, response_json, revoke, tokens};

mod store;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Login,
    Cancel,
    Retry,
    Logout,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Login,
    Timeout,
    Keychain,
    Reauthenticate,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Status {
    Checking,
    SignedOut,
    Waiting,
    SignedIn(String),
    Offline(String),
    Failed(Problem),
    SigningOut,
    RevocationUnconfirmed,
}

pub struct Controller {
    commands: Option<mpsc::Sender<Request>>,
    updates: mpsc::Receiver<Update>,
    epoch: u64,
    status: Status,
    cancel: Option<(Arc<LoginGate>, Status)>,
}

impl Controller {
    pub fn start() -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let commands =
            thread::Builder::new().name("kodma-account".into()).spawn(move || Worker::new(update_tx).run(&command_rx)).ok().map(|_| command_tx);
        let status = if commands.is_some() { Status::Checking } else { Status::Failed(Problem::Keychain) };
        Self { commands, updates: update_rx, epoch: 0, status, cancel: None }
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn poll(&mut self) {
        while let Ok((epoch, status)) = self.updates.try_recv() {
            if epoch == self.epoch {
                if status != Status::Waiting {
                    self.cancel = None;
                }
                self.status = status;
            }
        }
    }

    pub fn perform(&mut self, action: Action) {
        if action == Action::Cancel {
            if let Some((cancel, previous)) = self.cancel.take() {
                if cancel.cancel() {
                    eprintln!("Kodma Canvas account-test: cancelled");
                    self.epoch = self.epoch.wrapping_add(1);
                    self.status = previous;
                } else {
                    self.status = Status::Checking;
                }
            }
            return;
        }
        let (work, status) = match (action, &self.status) {
            (
                Action::Login,
                Status::SignedOut | Status::RevocationUnconfirmed | Status::Failed(Problem::Login | Problem::Timeout | Problem::Reauthenticate),
            ) => {
                let cancel = Arc::new(LoginGate::default());
                let previous = if self.status == Status::Failed(Problem::Reauthenticate) { self.status.clone() } else { Status::SignedOut };
                self.cancel = Some((cancel.clone(), previous));
                (Work::Login(cancel), Status::Waiting)
            }
            (Action::Retry, Status::Failed(Problem::Keychain) | Status::Offline(_)) => (Work::Retry, Status::Checking),
            (Action::Logout, Status::SignedIn(_) | Status::Offline(_) | Status::Failed(Problem::Reauthenticate)) => {
                (Work::Logout, Status::SigningOut)
            }
            _ => return,
        };
        self.epoch = self.epoch.wrapping_add(1);
        if self.commands.as_ref().is_some_and(|commands| commands.send((self.epoch, work)).is_ok()) {
            if action == Action::Logout {
                eprintln!("Kodma Canvas account-test: sign-out-requested");
            }
            self.status = status;
        } else {
            *self = Self::start();
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        if let Some((cancel, _)) = self.cancel.take() {
            cancel.cancel();
        }
    }
}

type Request = (u64, Work);
type Update = (u64, Status);

enum Work {
    Login(Arc<LoginGate>),
    Retry,
    Logout,
}

#[derive(Clone, Copy)]
enum Retry {
    Load,
    Refresh,
    Save,
    ForgetRejected,
    Logout,
}

#[derive(Deserialize, Serialize)]
struct Stored {
    identity: tokens::Identity,
    refresh_token: String,
    // Mark the request before it can leave the process. A crash or network error
    // during rotation must not replay the possibly spent token on the next launch.
    refresh_uncertain: bool,
}

impl Stored {
    fn display(&self) -> String {
        self.identity.email.as_ref().unwrap_or(&self.identity.sub).clone()
    }
}

struct Worker {
    updates: mpsc::Sender<Update>,
    store: Option<store::Store>,
    current: Option<Stored>,
    retry: Retry,
    refresh_at: Option<Instant>,
}

impl Worker {
    fn new(updates: mpsc::Sender<Update>) -> Self {
        Self { updates, store: None, current: None, retry: Retry::Load, refresh_at: None }
    }

    fn run(mut self, commands: &mpsc::Receiver<Request>) {
        let mut epoch = 0;
        let status = self.load();
        let _ = self.updates.send((epoch, status));

        loop {
            let command = match self.refresh_at {
                Some(deadline) => commands.recv_timeout(deadline.saturating_duration_since(Instant::now())),
                None => commands.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
            };
            let status = match command {
                Ok((id, action)) => {
                    epoch = id;
                    match action {
                        Work::Login(cancel) => self.sign_in(epoch, &cancel),
                        Work::Retry => self.retry(),
                        Work::Logout => self.forget(true),
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => self.refresh(),
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            };
            let _ = self.updates.send((epoch, status));
        }
    }

    fn load(&mut self) -> Status {
        self.retry = Retry::Load;
        let Ok(store) = store::Store::open() else {
            return Status::Failed(Problem::Keychain);
        };
        let loaded = store.load();
        self.store = Some(store);
        match loaded {
            Ok(Some(current)) => {
                self.current = Some(current);
                self.refresh()
            }
            Ok(None) => Status::SignedOut,
            Err(()) => Status::Failed(Problem::Keychain),
        }
    }

    fn retry(&mut self) -> Status {
        match self.retry {
            Retry::Load => {
                self.store = None;
                self.load()
            }
            Retry::Refresh => self.refresh(),
            Retry::Save => self.save(),
            Retry::ForgetRejected => self.forget(false),
            Retry::Logout => self.forget(true),
        }
    }

    fn sign_in(&mut self, epoch: u64, gate: &LoginGate) -> Status {
        let login = login(Some(gate)).and_then(|login| {
            let refresh_token = login.refresh_token.filter(|value| !value.is_empty()).ok_or(Error::MissingRefresh)?;
            Ok(Stored { identity: login.identity, refresh_token, refresh_uncertain: false })
        });
        match login {
            Ok(session) if gate.begin_commit() => {
                let _ = self.updates.send((epoch, Status::Checking));
                eprintln!("Kodma Canvas account-test: identity-verified");
                let old = self.current.replace(session);
                let status = self.save();
                if let Some(old) = old {
                    revoke_session(&old.refresh_token);
                }
                status
            }
            Ok(session) => {
                revoke_session(&session.refresh_token);
                Status::SignedOut
            }
            Err(Error::Cancelled) => {
                eprintln!("Kodma Canvas account-test: browser-cancelled");
                self.current.as_ref().map_or(Status::SignedOut, |_| Status::Failed(Problem::Reauthenticate))
            }
            Err(_) if self.current.is_some() => Status::Failed(Problem::Reauthenticate),
            Err(error) => {
                eprintln!("Kodma Canvas account-test: {error:?}");
                Status::Failed(if error == Error::Timeout { Problem::Timeout } else { Problem::Login })
            }
        }
    }

    fn save(&mut self) -> Status {
        self.retry = Retry::Save;
        let Some(current) = &self.current else {
            return Status::SignedOut;
        };
        let saved = self.store.as_ref().is_some_and(|store| store.save(current).is_ok());
        if !saved {
            return Status::Failed(Problem::Keychain);
        }
        eprintln!("Kodma Canvas account-test: session-saved");
        let seconds = tokens::now().map_or(240, |now| current.identity.expires_at.saturating_sub(now).saturating_sub(60));
        self.refresh_at = Some(Instant::now() + Duration::from_secs(seconds.clamp(5, 240)));
        Status::SignedIn(current.display())
    }

    fn forget(&mut self, revoke_on_server: bool) -> Status {
        self.refresh_at = None;
        self.retry = if revoke_on_server { Retry::Logout } else { Retry::ForgetRejected };
        if !self.store.as_ref().is_some_and(|store| store.delete().is_ok()) {
            return Status::Failed(Problem::Keychain);
        }
        let Some(current) = self.current.take() else {
            return Status::SignedOut;
        };
        if revoke_on_server && !revoke_session(&current.refresh_token) {
            eprintln!("Kodma Canvas account-test: signed-out-locally");
            return Status::RevocationUnconfirmed;
        }
        eprintln!("Kodma Canvas account-test: signed-out");
        Status::SignedOut
    }

    fn refresh(&mut self) -> Status {
        self.refresh_at = None;
        self.retry = Retry::Refresh;
        let (Some(current), Some(store)) = (&mut self.current, &self.store) else {
            return Status::SignedOut;
        };
        if current.refresh_uncertain {
            return Status::Failed(Problem::Reauthenticate);
        }
        let agent = agent();
        let Some((discovery, keys)) = prepare_refresh(&agent) else {
            return Status::Offline(current.display());
        };
        current.refresh_uncertain = true;
        if store.save(current).is_err() {
            current.refresh_uncertain = false;
            return Status::Failed(Problem::Keychain);
        }
        eprintln!("Kodma Canvas account-test: refresh-requested");
        match rotate(&agent, &discovery, &keys, current) {
            Rotation::Rotated => {
                eprintln!("Kodma Canvas account-test: refresh-verified");
                self.save()
            }
            Rotation::Rejected => self.forget(false),
            Rotation::Unconfirmed => Status::Failed(Problem::Reauthenticate),
        }
    }
}

fn prepare_refresh(agent: &ureq::Agent) -> Option<(Discovery, tokens::KeySet)> {
    // Check service reachability and fetch verification keys before attempting rotation.
    let discovery = Discovery::fetch(agent).ok()?;
    let keys = fetch_json(agent, discovery.jwks.as_str())?;
    Some((discovery, keys))
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
}

enum Rotation {
    Rotated,
    Rejected,
    Unconfirmed,
}

fn rotate(agent: &ureq::Agent, discovery: &Discovery, keys: &tokens::KeySet, current: &mut Stored) -> Rotation {
    let Ok(mut response) = agent.post(discovery.token.as_str()).send_form([
        ("grant_type", "refresh_token"),
        ("client_id", CLIENT_ID),
        ("refresh_token", &current.refresh_token),
    ]) else {
        return Rotation::Unconfirmed;
    };
    if response.status().as_u16() == 400 && read_json::<TokenError>(response.body_mut()).is_some_and(|body| body.error == "invalid_grant") {
        return Rotation::Rejected;
    }
    let Some(response) = response_json::<tokens::TokenResponse>(response) else {
        return Rotation::Unconfirmed;
    };
    let Ok(expires_at) = tokens::verify_refresh(&response, keys, &current.identity) else {
        return Rotation::Unconfirmed;
    };
    let Some(refresh_token) = response.refresh_token.filter(|next| !next.is_empty() && *next != current.refresh_token) else {
        return Rotation::Unconfirmed;
    };
    current.identity.expires_at = expires_at;
    current.refresh_token = refresh_token;
    current.refresh_uncertain = false;
    Rotation::Rotated
}

fn revoke_session(refresh_token: &str) -> bool {
    let agent = agent();
    Discovery::fetch(&agent).is_ok_and(|discovery| revoke(&agent, &discovery.revocation, refresh_token))
}

#[cfg(test)]
mod tests {
    use super::{Action, Controller, Request, Status, Update, Work, mpsc};

    fn harness(status: Status) -> (Controller, mpsc::Receiver<Request>, mpsc::Sender<Update>) {
        let (commands, command_rx) = mpsc::channel();
        let (update_tx, updates) = mpsc::channel();
        (Controller { commands: Some(commands), updates, epoch: 0, status, cancel: None }, command_rx, update_tx)
    }

    #[test]
    fn repeated_login_clicks_only_start_one_attempt_and_cancel_rejects_its_late_update() {
        let (mut ui, commands, updates) = harness(Status::SignedOut);
        ui.perform(Action::Login);
        ui.perform(Action::Login);
        let (id, Work::Login(gate)) = commands.try_recv().unwrap() else { panic!("expected sign-in") };
        assert!(commands.try_recv().is_err());
        ui.perform(Action::Cancel);
        assert!(gate.cancelled());
        assert!(!gate.begin_commit());
        updates.send((id, Status::SignedIn("stale".into()))).unwrap();
        ui.poll();
        assert!(ui.status() == &Status::SignedOut);
    }

    #[test]
    fn a_queued_refresh_status_cannot_restore_the_identity_after_logout() {
        let (mut ui, commands, updates) = harness(Status::SignedIn("test-account".into()));
        updates.send((0, Status::SignedIn("test-account".into()))).unwrap();
        ui.perform(Action::Logout);
        ui.poll();
        assert!(ui.status() == &Status::SigningOut);
        let (id, Work::Logout) = commands.try_recv().unwrap() else { panic!("expected logout") };
        updates.send((id, Status::SignedOut)).unwrap();
        ui.poll();
        assert!(ui.status() == &Status::SignedOut);
    }
}
