use std::collections::BTreeMap;

use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

                                                                       
                                                                          
                                                        
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationCondition {
    Fallback,
    Client(Uuid),
}

                                                                    
                 
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SettingEntry {
    pub activation: ActivationCondition,
    pub block: BlockRef,
}

                                                                           
                                                                          
                                                                 
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Settings {
    entries: BTreeMap<Uuid, Vec<SettingEntry>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum SettingsOperation {
                                                                
                                                                           
                  
    SetEntry {
        block_type: Uuid,
        activation: ActivationCondition,
        block: BlockRef,
    },
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

                                                                      
                   
    pub fn entries(&self, block_type: Uuid) -> &[SettingEntry] {
        self.entries.get(&block_type).map_or(&[], Vec::as_slice)
    }

                                                                          
                                                                     
                     
    pub fn resolve(&self, block_type: Uuid, client_id: Uuid) -> Option<BlockRef> {
        let entries = self.entries(block_type);
        entries
            .iter()
            .find(|entry| entry.activation == ActivationCondition::Client(client_id))
            .or_else(|| {
                entries
                    .iter()
                    .find(|entry| entry.activation == ActivationCondition::Fallback)
            })
            .map(|entry| entry.block)
    }
}

impl Block for Settings {
    type Operation = SettingsOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7365_7474_696e_6773_2d62_6c6f_636b_3031);

    fn apply_operation(settings: &mut Self, operation: &Self::Operation) {
        match operation {
            SettingsOperation::SetEntry {
                block_type,
                activation,
                block,
            } => {
                let entries = settings.entries.entry(*block_type).or_default();
                entries.retain(|entry| entry.activation != *activation);
                entries.push(SettingEntry {
                    activation: *activation,
                    block: *block,
                });
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.entries
            .values()
            .flatten()
            .filter_map(|entry| entry.block.as_direct())
            .collect()
    }
}

#[cfg(test)]
mod tests;
