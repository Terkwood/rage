//! Common helpers for CLI binaries.

use dialoguer::PasswordInput;
use rand::{
    distributions::{Distribution, Uniform},
    rngs::OsRng,
};
use secrecy::{ExposeSecret, SecretString};
use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;

use crate::keys::Identity;

pub mod file_io;

const BIP39_WORDLIST: &str = include_str!("../assets/bip39-english.txt");

/// Returns the age config directory.
///
/// Replicates the behaviour of [os.UserConfigDir] from Golang, which the
/// reference implementation uses. See [this issue] for more details.
///
/// [os.UserConfigDir]: https://golang.org/pkg/os/#UserConfigDir
/// [this issue]: https://github.com/FiloSottile/age/issues/15
pub fn get_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::data_dir()
    }

    #[cfg(not(target_os = "macos"))]
    {
        dirs::config_dir()
    }
}

/// Reads identities from the provided files if given, or the default system
/// locations if no files are given.
pub fn read_identities<E, F>(filenames: Vec<String>, no_default: F) -> Result<Vec<Identity>, E>
where
    E: From<io::Error>,
    F: FnOnce(&str) -> E,
{
    let mut identities = vec![];

    if filenames.is_empty() {
        let default_filename = get_config_dir()
            .map(|mut path| {
                path.push("age/keys.txt");
                path
            })
            .expect("an OS for which we know the default config directory");
        let f = File::open(&default_filename).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => no_default(default_filename.to_str().unwrap_or("")),
            _ => e.into(),
        })?;
        let buf = BufReader::new(f);
        identities.extend(Identity::from_buffer(buf)?);
    } else {
        for filename in filenames {
            identities.extend(Identity::from_file(filename)?);
        }
    }

    Ok(identities)
}

/// Reads a secret from stdin. If `confirm.is_some()` then an empty secret is allowed.
pub fn read_secret(prompt: &str, confirm: Option<&str>) -> io::Result<SecretString> {
    let mut input = PasswordInput::new();
    input.with_prompt(prompt);
    if let Some(confirm_prompt) = confirm {
        input
            .with_confirmation(confirm_prompt, "Inputs do not match")
            .allow_empty_password(true);
    }
    input.interact().map(SecretString::new)
}

/// A passphrase.
pub enum Passphrase {
    /// Typed by the user.
    Typed(SecretString),
    /// Generated.
    Generated(SecretString),
}

/// Reads a passphrase from stdin, or generates a secure one if none is provided.
pub fn read_or_generate_passphrase() -> io::Result<Passphrase> {
    let res = read_secret(
        "Type passphrase (leave empty to autogenerate a secure one)",
        Some("Confirm passphrase"),
    )?;

    if res.expose_secret().is_empty() {
        // Generate a secure passphrase
        let between = Uniform::from(0..2048);
        let mut rng = OsRng;
        let new_passphrase = (0..10)
            .map(|_| {
                BIP39_WORDLIST
                    .lines()
                    .nth(between.sample(&mut rng))
                    .expect("index is in range")
            })
            .fold(String::new(), |acc, s| {
                if acc.is_empty() {
                    acc + s
                } else {
                    acc + "-" + s
                }
            });
        Ok(Passphrase::Generated(SecretString::new(new_passphrase)))
    } else {
        Ok(Passphrase::Typed(res))
    }
}
