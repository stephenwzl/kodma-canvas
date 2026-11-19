//! The test application's one current session in the macOS login Keychain.
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use security_framework::passwords::{PasswordOptions, delete_generic_password, generic_password, set_generic_password};

use super::{CLIENT_ID, ISSUER, Stored};

const SERVICE: &str = "app.kodma.canvas.account";
const NOT_FOUND: i32 = -25300;

pub(super) struct Store {
    // A second copy of the app must not replay a refresh token this copy is rotating.
    _lock: File,
    account: String,
}

impl Store {
    pub fn open() -> Result<Self, ()> {
        let home = std::env::var_os("HOME").ok_or(())?;
        let dir = PathBuf::from(home).join("Library/Application Support/app.kodma.canvas");
        fs::create_dir_all(&dir).map_err(|_| ())?;
        let lock = OpenOptions::new().create(true).truncate(false).write(true).mode(0o600).open(dir.join("account-test.lock")).map_err(|_| ())?;
        lock.try_lock().map_err(|_| ())?;
        Ok(Self { _lock: lock, account: format!("{ISSUER} {CLIENT_ID}") })
    }

    pub fn load(&self) -> Result<Option<Stored>, ()> {
        match generic_password(PasswordOptions::new_generic_password(SERVICE, &self.account)) {
            Ok(value) => serde_json::from_slice(&value).map(Some).map_err(|_| ()),
            Err(error) if error.code() == NOT_FOUND => Ok(None),
            Err(_) => Err(()),
        }
    }

    pub fn save(&self, current: &Stored) -> Result<(), ()> {
        let value = serde_json::to_vec(current).map_err(|_| ())?;
        set_generic_password(SERVICE, &self.account, &value).map_err(|_| ())
    }

    pub fn delete(&self) -> Result<(), ()> {
        match delete_generic_password(SERVICE, &self.account) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == NOT_FOUND => Ok(()),
            Err(_) => Err(()),
        }
    }
}
