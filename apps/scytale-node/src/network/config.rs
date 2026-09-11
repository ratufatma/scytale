use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2pConfig {
    pub listen_addr: String,
    pub bootnodes: Vec<String>,
    pub key_path: PathBuf,
    pub known_peers_path: PathBuf,
    pub best_height: u64,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/9000".to_owned(),
            bootnodes: crate::config::DEFAULT_BOOTNODES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            key_path: PathBuf::from("./data/p2p_key.dat"),
            known_peers_path: PathBuf::from("./data/known_peers.json"),
            best_height: 0,
        }
    }
}

impl P2pConfig {
    pub fn for_data_dir(data_dir: impl AsRef<Path>) -> Self {
        Self {
            key_path: data_dir.as_ref().join("p2p_key.dat"),
            known_peers_path: data_dir.as_ref().join("known_peers.json"),
            ..Self::default()
        }
    }

    pub fn standalone() -> Self {
        Self {
            bootnodes: Vec::new(),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::P2pConfig;
    use std::path::Path;

    #[test]
    fn default_configuration_includes_default_bootnodes() {
        let config = P2pConfig::default();
        assert_eq!(config.listen_addr, "/ip4/0.0.0.0/tcp/9000");
        assert_eq!(
            config.bootnodes,
            crate::config::DEFAULT_BOOTNODES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(config.key_path, Path::new("./data/p2p_key.dat"));
    }

    #[test]
    fn standalone_configuration_has_no_bootnodes() {
        let config = P2pConfig::standalone();
        assert!(config.bootnodes.is_empty());
    }

    #[test]
    fn data_dir_controls_identity_location() {
        assert_eq!(
            P2pConfig::for_data_dir("/tmp/scytale").key_path,
            Path::new("/tmp/scytale/p2p_key.dat")
        );
        assert_eq!(
            P2pConfig::for_data_dir("/tmp/scytale").known_peers_path,
            Path::new("/tmp/scytale/known_peers.json")
        );
    }
}
