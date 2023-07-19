//! Units of execution.
// TODO - remove once schemars stops causing warning.
#![allow(clippy::field_reassign_with_default)]

use std::{cell::RefCell, rc::Rc};

use casper_storage::global_state::{shared::CorrelationId, storage::state::StateReader};
use casper_types::{
    bytesrepr::Bytes, contracts::NamedKeys, ContractHash, ContractPackageHash, ContractVersionKey,
    ExecutableDeployItem, Key, Package, Phase, ProtocolVersion, StoredValue,
};

use crate::core::{
    engine_state::{Error, ExecError},
    execution,
    tracking_copy::{TrackingCopy, TrackingCopyExt},
};

/// The type of execution about to be performed.
#[derive(Clone, Debug)]
pub(crate) enum ExecutionKind {
    /// Wasm bytes.
    Module(Bytes),
    /// Stored contract.
    Contract {
        /// Contract's hash.
        contract_hash: ContractHash,
        /// Entry point's name.
        entry_point_name: String,
    },
}

impl ExecutionKind {
    /// Returns a new module variant of `ExecutionKind`.
    pub fn new_module(module_bytes: Bytes) -> Self {
        ExecutionKind::Module(module_bytes)
    }

    /// Returns a new contract variant of `ExecutionKind`.
    pub fn new_contract(contract_hash: ContractHash, entry_point_name: String) -> Self {
        ExecutionKind::Contract {
            contract_hash,
            entry_point_name,
        }
    }

    /// Returns all the details necessary for execution.
    ///
    /// This object is generated based on information provided by [`ExecutableDeployItem`].
    pub fn new<R>(
        tracking_copy: Rc<RefCell<TrackingCopy<R>>>,
        named_keys: &NamedKeys,
        executable_deploy_item: ExecutableDeployItem,
        correlation_id: CorrelationId,
        protocol_version: &ProtocolVersion,
        phase: Phase,
    ) -> Result<ExecutionKind, Error>
    where
        R: StateReader<Key, StoredValue>,
        R::Error: Into<ExecError>,
    {
        let contract_hash: ContractHash;
        let mut contract_package: Package;

        let is_payment_phase = phase == Phase::Payment;

        match executable_deploy_item {
            ExecutableDeployItem::Transfer { .. } => {
                Err(Error::InvalidDeployItemVariant("Transfer".into()))
            }
            ExecutableDeployItem::ModuleBytes { module_bytes, .. }
                if module_bytes.is_empty() && is_payment_phase =>
            {
                Err(Error::InvalidDeployItemVariant(
                    "Empty module bytes for custom payment".into(),
                ))
            }
            ExecutableDeployItem::ModuleBytes { module_bytes, .. } => {
                Ok(ExecutionKind::new_module(module_bytes))
            }
            ExecutableDeployItem::StoredContractByHash {
                hash, entry_point, ..
            } => Ok(ExecutionKind::new_contract(hash, entry_point)),
            ExecutableDeployItem::StoredContractByName {
                name, entry_point, ..
            } => {
                let contract_key = named_keys.get(&name).cloned().ok_or_else(|| {
                    Error::Exec(execution::Error::NamedKeyNotFound(name.to_string()))
                })?;

                contract_hash =
                    ContractHash::new(contract_key.into_hash().ok_or(Error::InvalidKeyVariant)?);

                Ok(ExecutionKind::new_contract(contract_hash, entry_point))
            }
            ExecutableDeployItem::StoredVersionedContractByName {
                name,
                version,
                entry_point,
                ..
            } => {
                let contract_package_hash: ContractPackageHash = {
                    named_keys
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| {
                            Error::Exec(execution::Error::NamedKeyNotFound(name.to_string()))
                        })?
                        .into_hash()
                        .ok_or(Error::InvalidKeyVariant)?
                        .into()
                };

                contract_package = tracking_copy
                    .borrow_mut()
                    .get_contract_package(correlation_id, contract_package_hash)?;

                let maybe_version_key =
                    version.map(|ver| ContractVersionKey::new(protocol_version.value().major, ver));

                let contract_version_key = maybe_version_key
                    .or_else(|| contract_package.current_contract_version())
                    .ok_or(Error::Exec(execution::Error::NoActiveContractVersions(
                        contract_package_hash,
                    )))?;

                if !contract_package.is_version_enabled(contract_version_key) {
                    return Err(Error::Exec(execution::Error::InvalidContractVersion(
                        contract_version_key,
                    )));
                }

                let looked_up_contract_hash: ContractHash = contract_package
                    .lookup_contract_hash(contract_version_key)
                    .ok_or(Error::Exec(execution::Error::InvalidContractVersion(
                        contract_version_key,
                    )))?
                    .to_owned();

                Ok(ExecutionKind::new_contract(
                    looked_up_contract_hash,
                    entry_point,
                ))
            }
            ExecutableDeployItem::StoredVersionedContractByHash {
                hash: contract_package_hash,
                version,
                entry_point,
                ..
            } => {
                contract_package = tracking_copy
                    .borrow_mut()
                    .get_contract_package(correlation_id, contract_package_hash)?;

                let maybe_version_key =
                    version.map(|ver| ContractVersionKey::new(protocol_version.value().major, ver));

                let contract_version_key = maybe_version_key
                    .or_else(|| contract_package.current_contract_version())
                    .ok_or(Error::Exec(execution::Error::NoActiveContractVersions(
                        contract_package_hash,
                    )))?;

                if !contract_package.is_version_enabled(contract_version_key) {
                    return Err(Error::Exec(execution::Error::InvalidContractVersion(
                        contract_version_key,
                    )));
                }

                let looked_up_contract_hash = *contract_package
                    .lookup_contract_hash(contract_version_key)
                    .ok_or(Error::Exec(execution::Error::InvalidContractVersion(
                        contract_version_key,
                    )))?;

                Ok(ExecutionKind::new_contract(
                    looked_up_contract_hash,
                    entry_point,
                ))
            }
        }
    }
}
