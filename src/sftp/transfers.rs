use crate::models::TransferProgress;

pub fn merge_transfer(update: &TransferProgress, transfers: &mut Vec<TransferProgress>) {
    if let Some(existing) = transfers.iter_mut().find(|item| item.id == update.id) {
        *existing = update.clone();
    } else {
        transfers.push(update.clone());
    }

    transfers.sort_by(|left, right| right.label.cmp(&left.label));
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
}
