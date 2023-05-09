use std::{fs, path::PathBuf};

use anyhow::{anyhow, Result};
use blockifier::{
    execution::contract_class::ContractClass,
    transaction::{
        account_transaction::AccountTransaction,
        transaction_execution::Transaction as BlockifierTransaction,
        transactions::DeclareTransaction,
    },
};
use starknet::core::types::{contract::legacy::LegacyContractClass, FieldElement};
use starknet_api::{
    core::ClassHash,
    hash::StarkFelt,
    transaction::{
        DeployAccountTransaction, InvokeTransaction, InvokeTransactionV1, L1HandlerTransaction,
        Transaction,
    },
    StarknetApiError,
};

use anyhow::Ok;
use blockifier::execution::contract_class::{
    casm_contract_into_contract_class, ContractClass as BlockifierContractClass,
};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;

pub fn get_contract_class(contract_path: &str) -> ContractClass {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), contract_path].iter().collect();
    let raw_contract_class = fs::read_to_string(path).unwrap();
    serde_json::from_str(&raw_contract_class).unwrap()
}

pub fn convert_blockifier_tx_to_starknet_api_tx(
    transaction: &BlockifierTransaction,
) -> Transaction {
    match transaction {
        BlockifierTransaction::AccountTransaction(tx) => match tx {
            AccountTransaction::Invoke(tx) => {
                Transaction::Invoke(InvokeTransaction::V1(InvokeTransactionV1 {
                    nonce: tx.nonce(),
                    max_fee: tx.max_fee(),
                    calldata: tx.calldata(),
                    signature: tx.signature(),
                    sender_address: tx.sender_address(),
                    transaction_hash: tx.transaction_hash(),
                }))
            }
            AccountTransaction::DeployAccount(tx) => {
                Transaction::DeployAccount(DeployAccountTransaction {
                    nonce: tx.nonce,
                    max_fee: tx.max_fee,
                    version: tx.version,
                    class_hash: tx.class_hash,
                    signature: tx.signature.clone(),
                    transaction_hash: tx.transaction_hash,
                    contract_address: tx.contract_address,
                    contract_address_salt: tx.contract_address_salt,
                    constructor_calldata: tx.constructor_calldata.clone(),
                })
            }
            AccountTransaction::Declare(DeclareTransaction { tx, .. }) => match tx {
                starknet_api::transaction::DeclareTransaction::V0(tx) => {
                    Transaction::Declare(starknet_api::transaction::DeclareTransaction::V0(
                        starknet_api::transaction::DeclareTransactionV0V1 {
                            nonce: tx.nonce,
                            max_fee: tx.max_fee,
                            class_hash: tx.class_hash,
                            signature: tx.signature.clone(),
                            sender_address: tx.sender_address,
                            transaction_hash: tx.transaction_hash,
                        },
                    ))
                }

                starknet_api::transaction::DeclareTransaction::V1(tx) => {
                    Transaction::Declare(starknet_api::transaction::DeclareTransaction::V1(
                        starknet_api::transaction::DeclareTransactionV0V1 {
                            nonce: tx.nonce,
                            max_fee: tx.max_fee,
                            class_hash: tx.class_hash,
                            signature: tx.signature.clone(),
                            sender_address: tx.sender_address,
                            transaction_hash: tx.transaction_hash,
                        },
                    ))
                }

                starknet_api::transaction::DeclareTransaction::V2(tx) => {
                    Transaction::Declare(starknet_api::transaction::DeclareTransaction::V2(
                        starknet_api::transaction::DeclareTransactionV2 {
                            nonce: tx.nonce,
                            max_fee: tx.max_fee,
                            class_hash: tx.class_hash,
                            signature: tx.signature.clone(),
                            sender_address: tx.sender_address,
                            transaction_hash: tx.transaction_hash,
                            compiled_class_hash: tx.compiled_class_hash,
                        },
                    ))
                }
            },
        },
        BlockifierTransaction::L1HandlerTransaction(tx) => {
            Transaction::L1Handler(L1HandlerTransaction {
                nonce: tx.nonce,
                version: tx.version,
                calldata: tx.calldata.clone(),
                transaction_hash: tx.transaction_hash,
                contract_address: tx.contract_address,
                entry_point_selector: tx.entry_point_selector,
            })
        }
    }
}

pub fn compute_legacy_class_hash(contract_class_str: &str) -> Result<ClassHash> {
    let contract_class: LegacyContractClass = ::serde_json::from_str(contract_class_str)?;
    let seirra_class_hash = contract_class.class_hash()?;
    Ok(ClassHash(field_element_to_starkfelt(&seirra_class_hash)))
}

pub fn field_element_to_starkfelt(field_element: &FieldElement) -> StarkFelt {
    StarkFelt::new(field_element.to_bytes_be())
        .expect("must be able to convert to StarkFelt from FieldElement")
}

pub fn starkfelt_to_u128(felt: StarkFelt) -> Result<u128> {
    const COMPLIMENT_OF_U128: usize =
        std::mem::size_of::<StarkFelt>() - std::mem::size_of::<u128>();

    let (rest, u128_bytes) = felt.bytes().split_at(COMPLIMENT_OF_U128);
    if rest != [0u8; COMPLIMENT_OF_U128] {
        Err(anyhow!(StarknetApiError::OutOfRange {
            string: felt.to_string(),
        }))
    } else {
        Ok(u128::from_be_bytes(
            u128_bytes
                .try_into()
                .expect("u128_bytes should be of size usize."),
        ))
    }
}

pub fn blockifier_contract_class_from_flattened_sierra_class(
    raw_contract_class: &str,
) -> Result<BlockifierContractClass> {
    let value = serde_json::from_str::<serde_json::Value>(raw_contract_class)?;
    let contract_class = cairo_lang_starknet::contract_class::ContractClass {
        abi: serde_json::from_value(value["abi"].clone()).ok(),
        sierra_program: serde_json::from_value(value["sierra_program"].clone())?,
        entry_points_by_type: serde_json::from_value(value["entry_points_by_type"].clone())?,
        contract_class_version: serde_json::from_value(value["contract_class_version"].clone())?,
        sierra_program_debug_info: serde_json::from_value(
            value["sierra_program_debug_info"].clone(),
        )
        .ok(),
    };

    let casm_contract = CasmContractClass::from_contract_class(contract_class, true)?;
    Ok(casm_contract_into_contract_class(casm_contract)?)
}
