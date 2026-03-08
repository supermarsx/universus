#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum PaymentStatus {
    Pending,
    Completed,
    Failed,
    Refunded,
}

#[derive(Clone, Debug, Serialize)]
pub struct Transaction {
    pub id: i64,
    pub user_id: i64,
    pub amount_cents: i64,
    pub currency: String,
    pub description: String,
    pub status: PaymentStatus,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PaymentRequest {
    pub user_id: i64,
    pub amount_cents: i64,
    pub currency: String,
    pub description: String,
}

/// Placeholder payments provider adapter.
pub struct PaymentsProviderAdapter {
    next_id: i64,
    transactions: HashMap<i64, Transaction>,
}

impl PaymentsProviderAdapter {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            transactions: HashMap::new(),
        }
    }

    pub fn create_transaction(&mut self, request: PaymentRequest) -> Transaction {
        let txn = Transaction {
            id: self.next_id,
            user_id: request.user_id,
            amount_cents: request.amount_cents,
            currency: request.currency,
            description: request.description,
            status: PaymentStatus::Pending,
            created_at: String::new(),
        };
        self.transactions.insert(self.next_id, txn.clone());
        self.next_id += 1;
        txn
    }

    /// Completes a transaction if it is currently `Pending`.
    pub fn complete_transaction(&mut self, transaction_id: i64) -> bool {
        match self.transactions.get_mut(&transaction_id) {
            Some(txn) if txn.status == PaymentStatus::Pending => {
                txn.status = PaymentStatus::Completed;
                true
            }
            _ => false,
        }
    }

    /// Fails a transaction if it is currently `Pending`.
    pub fn fail_transaction(&mut self, transaction_id: i64) -> bool {
        match self.transactions.get_mut(&transaction_id) {
            Some(txn) if txn.status == PaymentStatus::Pending => {
                txn.status = PaymentStatus::Failed;
                true
            }
            _ => false,
        }
    }

    /// Refunds a transaction if it is currently `Completed`.
    pub fn refund_transaction(&mut self, transaction_id: i64) -> bool {
        match self.transactions.get_mut(&transaction_id) {
            Some(txn) if txn.status == PaymentStatus::Completed => {
                txn.status = PaymentStatus::Refunded;
                true
            }
            _ => false,
        }
    }

    pub fn get_transaction(&self, transaction_id: i64) -> Option<Transaction> {
        self.transactions.get(&transaction_id).cloned()
    }

    pub fn list_user_transactions(&self, user_id: i64) -> Vec<Transaction> {
        self.transactions
            .values()
            .filter(|t| t.user_id == user_id)
            .cloned()
            .collect()
    }

    /// Returns the sum of all `Completed` transaction amounts in cents.
    pub fn total_revenue_cents(&self) -> i64 {
        self.transactions
            .values()
            .filter(|t| t.status == PaymentStatus::Completed)
            .map(|t| t.amount_cents)
            .sum()
    }
}

impl Default for PaymentsProviderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(user_id: i64, amount: i64) -> PaymentRequest {
        PaymentRequest {
            user_id,
            amount_cents: amount,
            currency: "USD".to_string(),
            description: "test".to_string(),
        }
    }

    #[test]
    fn test_create_transaction() {
        let mut adapter = PaymentsProviderAdapter::new();
        let txn = adapter.create_transaction(req(1, 1000));
        assert_eq!(txn.id, 1);
        assert_eq!(txn.status, PaymentStatus::Pending);
    }

    #[test]
    fn test_auto_increment_id() {
        let mut adapter = PaymentsProviderAdapter::new();
        let t1 = adapter.create_transaction(req(1, 100));
        let t2 = adapter.create_transaction(req(1, 200));
        assert_eq!(t1.id, 1);
        assert_eq!(t2.id, 2);
    }

    #[test]
    fn test_complete_transaction() {
        let mut adapter = PaymentsProviderAdapter::new();
        let txn = adapter.create_transaction(req(1, 500));
        assert!(adapter.complete_transaction(txn.id));
        assert_eq!(
            adapter.get_transaction(txn.id).unwrap().status,
            PaymentStatus::Completed
        );
        // Cannot complete again
        assert!(!adapter.complete_transaction(txn.id));
    }

    #[test]
    fn test_fail_transaction() {
        let mut adapter = PaymentsProviderAdapter::new();
        let txn = adapter.create_transaction(req(1, 500));
        assert!(adapter.fail_transaction(txn.id));
        assert_eq!(
            adapter.get_transaction(txn.id).unwrap().status,
            PaymentStatus::Failed
        );
    }

    #[test]
    fn test_refund_transaction() {
        let mut adapter = PaymentsProviderAdapter::new();
        let txn = adapter.create_transaction(req(1, 500));
        // Cannot refund a pending transaction
        assert!(!adapter.refund_transaction(txn.id));
        adapter.complete_transaction(txn.id);
        assert!(adapter.refund_transaction(txn.id));
        assert_eq!(
            adapter.get_transaction(txn.id).unwrap().status,
            PaymentStatus::Refunded
        );
    }

    #[test]
    fn test_list_user_transactions() {
        let mut adapter = PaymentsProviderAdapter::new();
        adapter.create_transaction(req(1, 100));
        adapter.create_transaction(req(2, 200));
        adapter.create_transaction(req(1, 300));
        assert_eq!(adapter.list_user_transactions(1).len(), 2);
        assert_eq!(adapter.list_user_transactions(2).len(), 1);
    }

    #[test]
    fn test_total_revenue_cents() {
        let mut adapter = PaymentsProviderAdapter::new();
        let t1 = adapter.create_transaction(req(1, 1000));
        let t2 = adapter.create_transaction(req(1, 2000));
        let t3 = adapter.create_transaction(req(1, 500));
        adapter.complete_transaction(t1.id);
        adapter.complete_transaction(t2.id);
        adapter.fail_transaction(t3.id);
        assert_eq!(adapter.total_revenue_cents(), 3000);
    }

    #[test]
    fn test_get_nonexistent_transaction() {
        let adapter = PaymentsProviderAdapter::new();
        assert!(adapter.get_transaction(999).is_none());
    }
}
