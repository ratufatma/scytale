//! Initial Block Download state and progress tracking.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncState {
    Synced,
    Syncing {
        target_height: u64,
        current_height: u64,
    },
}

impl SyncState {
    pub fn is_syncing(&self) -> bool {
        matches!(self, Self::Syncing { .. })
    }

    pub fn observe_peer(&mut self, local_height: u64, peer_height: u64) -> bool {
        if peer_height <= local_height {
            return false;
        }

        match self {
            Self::Synced => {
                *self = Self::Syncing {
                    target_height: peer_height,
                    current_height: local_height,
                };
                true
            }
            Self::Syncing { target_height, .. } => {
                *target_height = (*target_height).max(peer_height);
                false
            }
        }
    }

    pub fn advance(&mut self, current_height: u64) -> bool {
        let Self::Syncing { target_height, .. } = self else {
            return false;
        };

        if current_height >= *target_height {
            *self = Self::Synced;
            true
        } else {
            if let Self::Syncing {
                current_height: state_height,
                ..
            } = self
            {
                *state_height = current_height;
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SyncState;

    #[test]
    fn peer_ahead_starts_sync_and_completion_returns_live() {
        let mut state = SyncState::Synced;
        assert!(state.observe_peer(10, 20));
        assert_eq!(
            state,
            SyncState::Syncing {
                target_height: 20,
                current_height: 10,
            }
        );
        assert!(!state.advance(19));
        assert!(state.advance(20));
        assert_eq!(state, SyncState::Synced);
    }

    #[test]
    fn higher_peer_extends_existing_target() {
        let mut state = SyncState::Syncing {
            target_height: 20,
            current_height: 10,
        };
        assert!(!state.observe_peer(10, 30));
        assert_eq!(
            state,
            SyncState::Syncing {
                target_height: 30,
                current_height: 10,
            }
        );
    }
}
