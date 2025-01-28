use arbitrary::{Arbitrary, Unstructured};
use futures::pin_mut;
use katana_chain_spec::ChainSpec;
use katana_executor::implementation::noop::NoopExecutorFactory;
use katana_primitives::transaction::{ExecutableTx, InvokeTx};
use katana_primitives::Felt;
use katana_provider::providers::db::DbProvider;
use tokio::time;

use super::*;
use crate::backend::gas_oracle::GasOracle;
use crate::backend::storage::Blockchain;

fn test_backend() -> Arc<Backend<NoopExecutorFactory>> {
    let chain_spec = Arc::new(ChainSpec::dev());
    let executor_factory = NoopExecutorFactory::new();
    let blockchain = Blockchain::new(DbProvider::new_ephemeral());
    let gas_oracle = GasOracle::fixed(Default::default(), Default::default());
    let backend = Arc::new(Backend::new(chain_spec, blockchain, gas_oracle, executor_factory));
    backend.init_genesis().expect("failed to initialize genesis");
    backend
}

#[tokio::test]
async fn interval_initial_state() {
    let backend = test_backend();
    let producer = IntervalBlockProducer::new(backend, Some(1000));

    assert!(producer.timer.is_none());
    assert!(producer.queued.is_empty());
    assert!(producer.ongoing_mining.is_none());
    assert!(producer.ongoing_execution.is_none());
}

#[tokio::test]
async fn interval_force_mine_without_transactions() {
    let backend = test_backend();

    let mut producer = IntervalBlockProducer::new(backend.clone(), None);
    producer.force_mine();

    let latest_num = backend.blockchain.provider().latest_number().unwrap();
    assert_eq!(latest_num, 1);
}

#[tokio::test]
async fn interval_mine_after_timer() {
    let backend = test_backend();
    let mut producer = IntervalBlockProducer::new(backend.clone(), Some(1000));
    // Initial state
    assert!(producer.timer.is_none());

    producer.queued.push_back(vec![dummy_transaction()]);

    let stream = producer;
    pin_mut!(stream);

    // Process the transaction, the timer should be automatically started
    let _ = stream.next().await;
    assert!(stream.timer.is_some());

    // Advance time to trigger mining
    time::sleep(Duration::from_secs(1)).await;
    let result = stream.next().await.expect("should mine block").unwrap();

    assert_eq!(result.block_number, 1);
    assert_eq!(backend.blockchain.provider().latest_number().unwrap(), 1);

    // Final state
    assert!(stream.timer.is_none());
}

// Helper functions to create test transactions
fn dummy_transaction() -> ExecutableTxWithHash {
    fn tx() -> ExecutableTx {
        let data = (0..InvokeTx::size_hint(0).0).map(|_| rand::random::<u8>()).collect::<Vec<u8>>();
        let mut unstructured = Unstructured::new(&data);
        ExecutableTx::Invoke(InvokeTx::arbitrary(&mut unstructured).unwrap())
    }

    ExecutableTxWithHash { hash: Felt::ONE, transaction: tx() }
}
