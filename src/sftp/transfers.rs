use crate::models::TransferProgress;

/// Finished transfers kept for display; older ones are dropped.
const MAX_FINISHED_TRANSFERS: usize = 20;

/// Applies a progress update. New transfers appear first; finished ones are
/// kept (up to a limit) so the user can see what happened.
pub fn merge_transfer(update: &TransferProgress, transfers: &mut Vec<TransferProgress>) {
    if let Some(existing) = transfers.iter_mut().find(|item| item.id == update.id) {
        *existing = update.clone();
    } else {
        transfers.insert(0, update.clone());
    }

    let mut finished = 0;
    transfers.retain(|item| {
        if !item.is_finished() {
            return true;
        }
        finished += 1;
        finished <= MAX_FINISHED_TRANSFERS
    });
}

pub fn clear_finished_transfers(transfers: &mut Vec<TransferProgress>) {
    transfers.retain(|item| !item.is_finished());
}

#[cfg(test)]
mod tests {
    use crate::models::{TransferDirection, TransferProgress, TransferStatus};

    #[test]
    fn removes_completed_transfers_from_the_active_list() {
        let mut transfers = Vec::new();
        let mut completed = TransferProgress::queued("empty.txt", TransferDirection::Upload, 0);
        completed.status = TransferStatus::Completed;

        super::merge_transfer(&completed, &mut transfers);

        assert_eq!(transfers.len(), 1);
        assert!(matches!(transfers[0].status, TransferStatus::Completed));
    }

    #[test]
    fn newest_transfer_is_listed_first_and_history_is_capped() {
        let mut transfers = Vec::new();
        for index in 0..30 {
            let mut transfer =
                TransferProgress::queued(format!("{index}.txt"), TransferDirection::Upload, 1);
            transfer.status = TransferStatus::Completed;
            super::merge_transfer(&transfer, &mut transfers);
        }
        let running = TransferProgress::queued("live.bin", TransferDirection::Download, 10);
        super::merge_transfer(&running, &mut transfers);

        assert_eq!(transfers[0].label, "live.bin");
        assert_eq!(transfers[1].label, "29.txt");
        assert_eq!(transfers.len(), 21);

        super::clear_finished_transfers(&mut transfers);
        assert_eq!(transfers.len(), 1);
    }
}
